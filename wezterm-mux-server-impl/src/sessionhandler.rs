use crate::PKI;
use anyhow::{anyhow, Context};
use codec::*;
use config::TermConfig;
use mux::client::ClientId;
use mux::domain::SplitSource;
use mux::output_signal::{RepaintWait, RepaintWaitOutcome};
use mux::pane::{CachePolicy, Pane, PaneId};
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::TabId;
use mux::{Mux, MuxNotification};
use promise::spawn::spawn_into_main_thread;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use termwiz::surface::SequenceNo;
use url::Url;
use wezterm_term::terminal::Alert;
use wezterm_term::{StableRowIndex, TerminalSize};

#[derive(Clone)]
pub struct PduSender {
    func: Arc<dyn Fn(DecodedPdu) -> anyhow::Result<()> + Send + Sync>,
}

impl PduSender {
    pub fn send(&self, pdu: DecodedPdu) -> anyhow::Result<()> {
        (self.func)(pdu)
    }

    pub fn new<T>(f: T) -> Self
    where
        T: Fn(DecodedPdu) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        Self { func: Arc::new(f) }
    }
}

#[derive(Default, Debug)]
pub(crate) struct PerPane {
    cursor_position: StableCursorPosition,
    title: String,
    working_dir: Option<Url>,
    dimensions: RenderableDimensions,
    mouse_grabbed: bool,
    alt_screen_active: bool,
    sent_initial_palette: bool,
    seqno: SequenceNo,
    config_generation: usize,
    pub(crate) notifications: Vec<Alert>,
    /// Number of resizes of this pane that are waiting for it to redraw
    /// before replying. Pushes are held back while this is non-zero.
    resize_repaint_waits: usize,
    /// A push task for this pane is queued on the main thread and hasn't
    /// started yet. Output that arrives meanwhile is covered by that push,
    /// so no second task is queued.
    push_scheduled: bool,
}

/// Counts a resize wait in `PerPane::resize_repaint_waits` while alive.
struct ResizeRepaintWaitGuard {
    per_pane: Arc<Mutex<PerPane>>,
}

impl ResizeRepaintWaitGuard {
    fn new(per_pane: &Arc<Mutex<PerPane>>) -> Self {
        per_pane.lock().unwrap().resize_repaint_waits += 1;
        Self {
            per_pane: Arc::clone(per_pane),
        }
    }
}

impl Drop for ResizeRepaintWaitGuard {
    fn drop(&mut self) {
        self.per_pane.lock().unwrap().resize_repaint_waits -= 1;
    }
}

/// The longest a client may ask us to wait for a pane to redraw after a
/// resize.
const MAX_RESIZE_REPAINT_WAIT: Duration = Duration::from_millis(100);

/// Wait for the pane behind `mark` to redraw after a resize at `resized`.
/// The wait blocks a thread from smol's blocking pool, not the caller's.
async fn wait_for_repaint(
    mark: Option<RepaintWait>,
    resized: Instant,
    budget: Duration,
) -> ResizeRepaintStop {
    let Some(mark) = mark else {
        return ResizeRepaintStop::NoSignal;
    };
    match smol::unblock(move || mark.wait(resized, resized + budget)).await {
        RepaintWaitOutcome::Quiet => ResizeRepaintStop::Quiet,
        RepaintWaitOutcome::SyncEnd => ResizeRepaintStop::SyncEnd,
        RepaintWaitOutcome::NoOutput => ResizeRepaintStop::NoOutput,
        RepaintWaitOutcome::Budget => ResizeRepaintStop::Budget,
    }
}

/// Handle a `Resize` request. Runs on the main thread. When the client asks
/// for a repaint wait, the wait happens on a blocking thread so the main
/// thread keeps serving other clients and panes; the changes are then
/// computed back on the main thread and returned in the reply.
async fn resize_pane(
    containing_tab_id: TabId,
    pane_id: PaneId,
    size: TerminalSize,
    repaint_wait_ms: u64,
    per_pane: Arc<Mutex<PerPane>>,
) -> anyhow::Result<Pdu> {
    let mux = Mux::get();
    let pane = mux
        .get_pane(pane_id)
        .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;

    // Take the output mark and hold back pushes before resizing, so that
    // both cover everything the program writes in response.
    let wait = if repaint_wait_ms > 0 {
        let guard = ResizeRepaintWaitGuard::new(&per_pane);
        let mark = mux
            .pane_output_signal(pane_id)
            .map(|signal| signal.begin_wait());
        Some((guard, mark))
    } else {
        None
    };

    pane.resize(size)?;
    let resized = Instant::now();
    let tab = mux
        .get_tab(containing_tab_id)
        .ok_or_else(|| anyhow!("no such tab {}", containing_tab_id))?;
    tab.rebuild_splits_sizes_from_contained_panes();
    drop(tab);
    drop(pane);
    drop(mux);

    let Some((guard, mark)) = wait else {
        return Ok(Pdu::UnitResponse(UnitResponse {}));
    };

    let budget = Duration::from_millis(repaint_wait_ms).min(MAX_RESIZE_REPAINT_WAIT);
    let stopped = wait_for_repaint(mark, resized, budget).await;
    let waited = resized.elapsed();

    let mux = Mux::get();
    let pane = mux
        .get_pane(pane_id)
        .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
    let changes = {
        let mut per_pane = per_pane.lock().unwrap();
        per_pane.compute_changes(&pane, None)
    };
    // Release the hold on pushes only after computing the changes, so
    // that a push held back during the wait is covered by this reply.
    drop(guard);

    log::trace!(
        "resize repaint wait: pane {pane_id} {}x{} waited {waited:?} \
         (budget {budget:?}), stopped: {}, {} lines",
        size.cols,
        size.rows,
        stopped.as_str(),
        changes.as_ref().map(|c| c.bonus_lines.len()).unwrap_or(0),
    );

    Ok(Pdu::ResizeRepaintResponse(ResizeRepaintResponse {
        pane_id,
        stopped,
        waited_us: waited.as_micros() as u64,
        changes,
    }))
}

impl PerPane {
    fn compute_changes(
        &mut self,
        pane: &Arc<dyn Pane>,
        force_with_input_serial: Option<InputSerial>,
    ) -> Option<GetPaneRenderChangesResponse> {
        let mut changed = false;
        let mouse_grabbed = pane.is_mouse_grabbed();
        if mouse_grabbed != self.mouse_grabbed {
            changed = true;
        }

        let alt_screen_active = pane.is_alt_screen_active();
        if alt_screen_active != self.alt_screen_active {
            changed = true;
        }

        let dims = pane.get_dimensions();
        if dims != self.dimensions {
            changed = true;
        }

        let cursor_position = pane.get_cursor_position();
        if cursor_position != self.cursor_position {
            changed = true;
        }

        let title = pane.get_title();
        if title != self.title {
            changed = true;
        }

        let working_dir = pane.get_current_working_dir(CachePolicy::AllowStale);
        if working_dir != self.working_dir {
            changed = true;
        }

        let old_seqno = self.seqno;
        self.seqno = pane.get_current_seqno();
        let mut all_dirty_lines = pane.get_changed_since(
            0..dims.physical_top + dims.viewport_rows as StableRowIndex,
            old_seqno,
        );
        if !all_dirty_lines.is_empty() {
            changed = true;
        }

        if !changed && !force_with_input_serial.is_some() {
            return None;
        }

        // Figure out what we're going to send as dirty lines vs bonus lines
        let viewport_range =
            dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex;

        let (first_line, lines) = pane.get_lines(viewport_range);
        let mut bonus_lines = lines
            .into_iter()
            .enumerate()
            .filter_map(|(idx, mut line)| {
                let stable_row = first_line + idx as StableRowIndex;
                if all_dirty_lines.contains(stable_row) {
                    all_dirty_lines.remove(stable_row);
                    line.compress_for_scrollback();
                    Some((stable_row, line))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Always send the cursor's row, as that tends to the busiest and we don't
        // have a sequencing concept for our idea of the remote state.
        let (cursor_line_idx, mut lines) = pane.get_lines(cursor_position.y..cursor_position.y + 1);
        let mut cursor_line = lines.remove(0);
        cursor_line.compress_for_scrollback();
        bonus_lines.push((cursor_line_idx, cursor_line));

        self.cursor_position = cursor_position;
        self.title = title.clone();
        self.working_dir = working_dir.clone();
        self.dimensions = dims;
        self.mouse_grabbed = mouse_grabbed;
        self.alt_screen_active = alt_screen_active;

        let bonus_lines = bonus_lines.into();
        Some(GetPaneRenderChangesResponse {
            pane_id: pane.pane_id(),
            mouse_grabbed,
            alt_screen_active,
            dirty_lines: all_dirty_lines.iter().cloned().collect(),
            dimensions: dims,
            cursor_position,
            title,
            bonus_lines,
            working_dir: working_dir.map(Into::into),
            input_serial: force_with_input_serial,
            seqno: self.seqno,
        })
    }
}

fn maybe_push_pane_changes(
    pane: &Arc<dyn Pane>,
    sender: PduSender,
    per_pane: Arc<Mutex<PerPane>>,
) -> anyhow::Result<()> {
    let mut per_pane = per_pane.lock().unwrap();
    if per_pane.resize_repaint_waits > 0 {
        // A resize is waiting for this pane to redraw; its reply will
        // carry these changes. Pushing them now would let them reach
        // the client before the reply, which the client then applies
        // after it has painted.
        log::trace!(
            "resize repaint wait: pane {} holding back push",
            pane.pane_id()
        );
    } else if let Some(resp) = per_pane.compute_changes(pane, None) {
        sender.send(DecodedPdu {
            pdu: Pdu::GetPaneRenderChangesResponse(resp),
            serial: 0,
        })?;
    }

    let config = config::configuration();
    if per_pane.config_generation != config.generation() {
        per_pane.config_generation = config.generation();
        // If the config changed, it may have changed colors
        // in the palette that we need to push down, so we
        // synthesize a palette change notification to let
        // the client know
        per_pane.notifications.push(Alert::PaletteChanged);
        per_pane.sent_initial_palette = true;
    }

    if !per_pane.sent_initial_palette {
        per_pane.notifications.push(Alert::PaletteChanged);
        per_pane.sent_initial_palette = true;
    }
    for alert in per_pane.notifications.drain(..) {
        match alert {
            Alert::PaletteChanged => {
                sender.send(DecodedPdu {
                    pdu: Pdu::SetPalette(SetPalette {
                        pane_id: pane.pane_id(),
                        palette: pane.palette(),
                    }),
                    serial: 0,
                })?;
            }
            alert => {
                sender.send(DecodedPdu {
                    pdu: Pdu::NotifyAlert(NotifyAlert {
                        pane_id: pane.pane_id(),
                        alert,
                    }),
                    serial: 0,
                })?;
            }
        }
    }
    Ok(())
}

/// Note that a push for this pane is queued. Returns false if one already
/// was, in which case the caller must not queue another.
fn mark_push_scheduled(per_pane: &Arc<Mutex<PerPane>>) -> bool {
    let mut per_pane = per_pane.lock().unwrap();
    if per_pane.push_scheduled {
        false
    } else {
        per_pane.push_scheduled = true;
        true
    }
}

pub struct SessionHandler {
    to_write_tx: PduSender,
    per_pane: HashMap<TabId, Arc<Mutex<PerPane>>>,
    client_id: Option<Arc<ClientId>>,
    proxy_client_id: Option<ClientId>,
}

impl Drop for SessionHandler {
    fn drop(&mut self) {
        if let Some(client_id) = self.client_id.take() {
            let mux = Mux::get();
            mux.unregister_client(&client_id);
        }
    }
}

impl SessionHandler {
    pub fn new(to_write_tx: PduSender) -> Self {
        Self {
            to_write_tx,
            per_pane: HashMap::new(),
            client_id: None,
            proxy_client_id: None,
        }
    }

    pub(crate) fn per_pane(&mut self, pane_id: PaneId) -> Arc<Mutex<PerPane>> {
        Arc::clone(
            self.per_pane
                .entry(pane_id)
                .or_insert_with(|| Arc::new(Mutex::new(PerPane::default()))),
        )
    }

    pub fn schedule_pane_push(&mut self, pane_id: PaneId) {
        let sender = self.to_write_tx.clone();
        let per_pane = self.per_pane(pane_id);
        // A pane that floods output notifies far faster than a push can be
        // computed and sent. Queuing a task per notification let the main
        // thread's queue grow without bound, starving every other request.
        if !mark_push_scheduled(&per_pane) {
            return;
        }
        spawn_into_main_thread(async move {
            // Clear the flag before computing, so that output arriving
            // while this push runs queues the next one.
            per_pane.lock().unwrap().push_scheduled = false;
            let mux = Mux::get();
            let pane = mux
                .get_pane(pane_id)
                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
            maybe_push_pane_changes(&pane, sender, per_pane)?;
            Ok::<(), anyhow::Error>(())
        })
        .detach();
    }

    pub fn process_one(&mut self, decoded: DecodedPdu) {
        let start = Instant::now();
        let sender = self.to_write_tx.clone();
        let serial = decoded.serial;

        if let Some(client_id) = &self.client_id {
            if decoded.pdu.is_user_input() {
                Mux::get().client_had_input(client_id);
            }
        }

        let send_response = move |result: anyhow::Result<Pdu>| {
            let pdu = match result {
                Ok(pdu) => pdu,
                Err(err) => Pdu::ErrorResponse(ErrorResponse {
                    reason: format!("Error: {err:#}"),
                }),
            };
            log::trace!("{} processing time {:?}", serial, start.elapsed());
            sender.send(DecodedPdu { pdu, serial }).ok();
        };

        fn catch<F, SND>(f: F, send_response: SND)
        where
            F: FnOnce() -> anyhow::Result<Pdu>,
            SND: Fn(anyhow::Result<Pdu>),
        {
            send_response(f());
        }

        match decoded.pdu {
            Pdu::Ping(Ping {}) => send_response(Ok(Pdu::Pong(Pong {}))),
            Pdu::SetWindowWorkspace(SetWindowWorkspace {
                window_id,
                workspace,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let mut window = mux
                                .get_window_mut(window_id)
                                .ok_or_else(|| anyhow!("window {} is invalid", window_id))?;
                            window.set_workspace(&workspace);
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::SetClientId(SetClientId {
                mut client_id,
                is_proxy,
            }) => {
                if is_proxy {
                    if self.proxy_client_id.is_none() {
                        // Copy proxy identity, but don't assign it to the mux;
                        // we'll use it to annotate the actual clients own
                        // identity when they send it
                        self.proxy_client_id.replace(client_id);
                    }
                } else {
                    // If this session is a proxy, override the incoming id with
                    // the proxy information so that it is clear what is going
                    // on from the `wezterm cli list-clients` information
                    if let Some(proxy_id) = &self.proxy_client_id {
                        client_id.ssh_auth_sock = proxy_id.ssh_auth_sock.clone();
                        // Note that this `via proxy pid` string is coupled
                        // with the logic in mux/src/ssh_agent
                        client_id.hostname =
                            format!("{} (via proxy pid {})", client_id.hostname, proxy_id.pid);
                    }

                    let client_id = Arc::new(client_id);
                    self.client_id.replace(client_id.clone());
                    spawn_into_main_thread(async move {
                        let mux = Mux::get();
                        mux.register_client(client_id);
                    })
                    .detach();
                }
                send_response(Ok(Pdu::UnitResponse(UnitResponse {})))
            }
            Pdu::SetFocusedPane(SetFocusedPane { pane_id }) => {
                let client_id = self.client_id.clone();
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let _identity = mux.with_identity(client_id);

                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow::anyhow!("pane {pane_id} not found"))?;

                            let (_domain_id, window_id, tab_id) = mux
                                .resolve_pane_id(pane_id)
                                .ok_or_else(|| anyhow::anyhow!("pane {pane_id} not found"))?;
                            {
                                let mut window =
                                    mux.get_window_mut(window_id).ok_or_else(|| {
                                        anyhow::anyhow!("window {window_id} not found")
                                    })?;
                                let tab_idx =
                                    window.get_tab_idx_for_id(tab_id).ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "tab {tab_id} isn't really in window {window_id}!?"
                                        )
                                    })?;
                                window.remember_and_set_active_tab_idx(tab_idx);
                            }
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow::anyhow!("tab {tab_id} not found"))?;
                            tab.set_active_pane(&pane);

                            mux.record_focus_for_current_identity(pane_id);
                            mux.notify(mux::MuxNotification::PaneFocused(pane_id));

                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::GetClientList(GetClientList) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let clients = mux.iter_clients();
                            Ok(Pdu::GetClientListResponse(GetClientListResponse {
                                clients,
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::ListPanes(ListPanes {}) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let mut tabs = vec![];
                            let mut tab_titles = vec![];
                            let mut window_titles = HashMap::new();
                            for window_id in mux.iter_windows().into_iter() {
                                let window = mux.get_window(window_id).unwrap();
                                window_titles.insert(window_id, window.get_title().to_string());
                                for tab in window.iter_tabs() {
                                    tabs.push(tab.codec_pane_tree());
                                    tab_titles.push(tab.get_title());
                                }
                            }
                            log::trace!("ListPanes {tabs:#?} {tab_titles:?}");
                            Ok(Pdu::ListPanesResponse(ListPanesResponse {
                                tabs,
                                tab_titles,
                                window_titles,
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::RenameWorkspace(RenameWorkspace {
                old_workspace,
                new_workspace,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            mux.rename_workspace(&old_workspace, &new_workspace);
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    );
                })
                .detach();
            }

            Pdu::WriteToPane(WriteToPane { pane_id, data }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.writer().write_all(&data)?;
                            maybe_push_pane_changes(&pane, sender, per_pane)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    );
                })
                .detach();
            }
            Pdu::EraseScrollbackRequest(EraseScrollbackRequest {
                pane_id,
                erase_mode,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.erase_scrollback(erase_mode);
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    );
                })
                .detach();
            }
            Pdu::KillPane(KillPane { pane_id }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.kill();
                            mux.remove_pane(pane_id);
                            maybe_push_pane_changes(&pane, sender, per_pane)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    );
                })
                .detach();
            }
            Pdu::SendPaste(SendPaste { pane_id, data }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.send_paste(&data)?;
                            maybe_push_pane_changes(&pane, sender, per_pane)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::SearchScrollbackRequest(SearchScrollbackRequest {
                pane_id,
                pattern,
                range,
                limit,
            }) => {
                use mux::pane::Pattern;

                async fn do_search(
                    pane_id: TabId,
                    pattern: Pattern,
                    range: std::ops::Range<StableRowIndex>,
                    limit: Option<u32>,
                ) -> anyhow::Result<Pdu> {
                    let mux = Mux::get();
                    let pane = mux
                        .get_pane(pane_id)
                        .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;

                    pane.search(pattern, range, limit).await.map(|results| {
                        Pdu::SearchScrollbackResponse(SearchScrollbackResponse { results })
                    })
                }

                spawn_into_main_thread(async move {
                    promise::spawn::spawn(async move {
                        let result = do_search(pane_id, pattern, range, limit).await;
                        send_response(result);
                    })
                    .detach();
                })
                .detach();
            }

            Pdu::SetPaneZoomed(SetPaneZoomed {
                containing_tab_id,
                pane_id,
                zoomed,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            let tab = mux
                                .get_tab(containing_tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", containing_tab_id))?;
                            match tab.get_zoomed_pane() {
                                Some(p) => {
                                    let is_zoomed = p.pane_id() == pane_id;
                                    if is_zoomed != zoomed {
                                        tab.set_zoomed(false);
                                        if zoomed {
                                            tab.set_active_pane(&pane);
                                            tab.set_zoomed(zoomed);
                                        }
                                    }
                                }
                                None => {
                                    if zoomed {
                                        tab.set_active_pane(&pane);
                                        tab.set_zoomed(zoomed);
                                    }
                                }
                            }
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::GetPaneDirection(GetPaneDirection { pane_id, direction }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let (_domain_id, _window_id, tab_id) = mux
                                .resolve_pane_id(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            let panes = tab.iter_panes_ignoring_zoom();
                            let pane_id = tab
                                .get_pane_direction(direction, true)
                                .map(|pane_index| panes[pane_index].pane.pane_id());

                            Ok(Pdu::GetPaneDirectionResponse(GetPaneDirectionResponse {
                                pane_id,
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::ActivatePaneDirection(ActivatePaneDirection { pane_id, direction }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let (_domain_id, _window_id, tab_id) = mux
                                .resolve_pane_id(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            tab.activate_pane_direction(direction);
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::Resize(Resize {
                containing_tab_id,
                pane_id,
                size,
                repaint_wait_ms,
            }) => {
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    send_response(
                        resize_pane(containing_tab_id, pane_id, size, repaint_wait_ms, per_pane)
                            .await,
                    );
                })
                .detach();
            }

            Pdu::SendKeyDown(SendKeyDown {
                pane_id,
                event,
                input_serial,
            }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.key_down(event.key, event.modifiers)?;

                            // For a key press, we want to always send back the
                            // cursor position so that the predictive echo doesn't
                            // leave the cursor in the wrong place
                            let mut per_pane = per_pane.lock().unwrap();
                            if let Some(resp) = per_pane.compute_changes(&pane, Some(input_serial))
                            {
                                sender.send(DecodedPdu {
                                    pdu: Pdu::GetPaneRenderChangesResponse(resp),
                                    serial: 0,
                                })?;
                            }
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::SendMouseEvent(SendMouseEvent { pane_id, event }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            pane.mouse_event(event)?;
                            maybe_push_pane_changes(&pane, sender, per_pane)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::SpawnV2(spawn) => {
                let client_id = self.client_id.clone();
                spawn_into_main_thread(async move {
                    schedule_domain_spawn_v2(spawn, send_response, client_id);
                })
                .detach();
            }

            Pdu::SplitPane(split) => {
                let client_id = self.client_id.clone();
                spawn_into_main_thread(async move {
                    schedule_split_pane(split, send_response, client_id);
                })
                .detach();
            }

            Pdu::MovePaneToNewTab(request) => {
                let client_id = self.client_id.clone();
                spawn_into_main_thread(async move {
                    schedule_move_pane(request, send_response, client_id);
                })
                .detach();
            }

            Pdu::GetPaneRenderableDimensions(GetPaneRenderableDimensions { pane_id }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            let cursor_position = pane.get_cursor_position();
                            let dimensions = pane.get_dimensions();
                            Ok(Pdu::GetPaneRenderableDimensionsResponse(
                                GetPaneRenderableDimensionsResponse {
                                    pane_id,
                                    cursor_position,
                                    dimensions,
                                },
                            ))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::GetPaneRenderChanges(GetPaneRenderChanges { pane_id, .. }) => {
                let sender = self.to_write_tx.clone();
                let per_pane = self.per_pane(pane_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let is_alive = match mux.get_pane(pane_id) {
                                Some(pane) => {
                                    maybe_push_pane_changes(&pane, sender, per_pane)?;
                                    true
                                }
                                None => false,
                            };
                            Ok(Pdu::LivenessResponse(LivenessResponse {
                                pane_id,
                                is_alive,
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::GetLines(GetLines { pane_id, lines }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;
                            let mut lines_and_indices = vec![];

                            for range in lines {
                                let (first_row, lines) = pane.get_lines(range);
                                for (idx, mut line) in lines.into_iter().enumerate() {
                                    let stable_row = first_row + idx as StableRowIndex;
                                    line.compress_for_scrollback();
                                    lines_and_indices.push((stable_row, line));
                                }
                            }
                            Ok(Pdu::GetLinesResponse(GetLinesResponse {
                                pane_id,
                                lines: lines_and_indices.into(),
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::GetImageCell(GetImageCell {
                pane_id,
                line_idx,
                cell_idx,
                data_hash,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let mut data = None;

                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;

                            let (_, lines) = pane.get_lines(line_idx..line_idx + 1);
                            'found_data: for line in lines {
                                if let Some(cell) = line.get_cell(cell_idx) {
                                    if let Some(images) = cell.attrs().images() {
                                        for im in images {
                                            if im.image_data().hash() == data_hash {
                                                data.replace(im.image_data().clone());
                                                break 'found_data;
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(Pdu::GetImageCellResponse(GetImageCellResponse {
                                pane_id,
                                data,
                            }))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::GetCodecVersion(_) => {
                match std::env::current_exe().context("resolving current_exe") {
                    Err(err) => send_response(Err(err)),
                    Ok(executable_path) => {
                        send_response(Ok(Pdu::GetCodecVersionResponse(GetCodecVersionResponse {
                            codec_vers: CODEC_VERSION,
                            version_string: config::wezterm_version().to_owned(),
                            executable_path,
                            config_file_path: std::env::var_os("WEZTERM_CONFIG_FILE")
                                .map(Into::into),
                        })))
                    }
                }
            }

            Pdu::GetTlsCreds(_) => {
                catch(
                    move || {
                        let client_cert_pem = PKI.generate_client_cert()?;
                        let ca_cert_pem = PKI.ca_pem_string()?;
                        Ok(Pdu::GetTlsCredsResponse(GetTlsCredsResponse {
                            client_cert_pem,
                            ca_cert_pem,
                        }))
                    },
                    send_response,
                );
            }
            Pdu::WindowTitleChanged(WindowTitleChanged { window_id, title }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let mut window = mux
                                .get_window_mut(window_id)
                                .ok_or_else(|| anyhow!("no such window {window_id}"))?;

                            window.set_title(&title);

                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::TabTitleChanged(TabTitleChanged { tab_id, title }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {tab_id}"))?;

                            tab.set_title(&title);

                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }
            Pdu::SetPalette(SetPalette { pane_id, palette }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let pane = mux
                                .get_pane(pane_id)
                                .ok_or_else(|| anyhow!("no such pane {}", pane_id))?;

                            match pane.get_config() {
                                Some(config) => match config.downcast_ref::<TermConfig>() {
                                    Some(tc) => tc.set_client_palette(palette),
                                    None => {
                                        log::error!(
                                            "pane {pane_id} doesn't \
                                            have TermConfig as its config! \
                                            Ignoring client palette update"
                                        );
                                    }
                                },
                                None => {
                                    let config = TermConfig::new();
                                    config.set_client_palette(palette);
                                    pane.set_config(Arc::new(config));
                                }
                            }

                            mux.notify(MuxNotification::Alert {
                                pane_id,
                                alert: Alert::PaletteChanged,
                            });

                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::AdjustPaneSize(AdjustPaneSize {
                pane_id,
                direction,
                amount,
            }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get();
                            let (_pane_domain_id, _window_id, tab_id) = mux
                                .resolve_pane_id(pane_id)
                                .ok_or_else(|| anyhow!("pane_id {} invalid", pane_id))?;

                            let tab = match mux.get_tab(tab_id) {
                                Some(tab) => tab,
                                None => {
                                    return Err(anyhow!(
                                        "Failed to retrieve tab with ID {}",
                                        tab_id
                                    ))
                                }
                            };

                            tab.adjust_pane_size(direction, amount);
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                })
                .detach();
            }

            Pdu::Invalid { .. } => send_response(Err(anyhow!("invalid PDU {:?}", decoded.pdu))),
            Pdu::Pong { .. }
            | Pdu::ListPanesResponse { .. }
            | Pdu::SetClipboard { .. }
            | Pdu::NotifyAlert { .. }
            | Pdu::SpawnResponse { .. }
            | Pdu::GetPaneRenderChangesResponse { .. }
            | Pdu::UnitResponse { .. }
            | Pdu::LivenessResponse { .. }
            | Pdu::GetPaneDirectionResponse { .. }
            | Pdu::ResizeRepaintResponse { .. }
            | Pdu::SearchScrollbackResponse { .. }
            | Pdu::GetLinesResponse { .. }
            | Pdu::GetCodecVersionResponse { .. }
            | Pdu::WindowWorkspaceChanged { .. }
            | Pdu::GetTlsCredsResponse { .. }
            | Pdu::GetClientListResponse { .. }
            | Pdu::PaneRemoved { .. }
            | Pdu::PaneFocused { .. }
            | Pdu::TabResized { .. }
            | Pdu::GetImageCellResponse { .. }
            | Pdu::MovePaneToNewTabResponse { .. }
            | Pdu::TabAddedToWindow { .. }
            | Pdu::GetPaneRenderableDimensionsResponse { .. }
            | Pdu::ErrorResponse { .. } => {
                send_response(Err(anyhow!("expected a request, got {:?}", decoded.pdu)))
            }
        }
    }
}

// Dancing around a little bit here; we can't directly spawn_into_main_thread the domain_spawn
// function below because the compiler thinks that all of its locals then need to be Send.
// We need to shimmy through this helper to break that aspect of the compiler flow
// analysis and allow things to compile.
fn schedule_domain_spawn_v2<SND>(
    spawn: SpawnV2,
    send_response: SND,
    client_id: Option<Arc<ClientId>>,
) where
    SND: Fn(anyhow::Result<Pdu>) + 'static,
{
    promise::spawn::spawn(async move { send_response(domain_spawn_v2(spawn, client_id).await) })
        .detach();
}

fn schedule_split_pane<SND>(split: SplitPane, send_response: SND, client_id: Option<Arc<ClientId>>)
where
    SND: Fn(anyhow::Result<Pdu>) + 'static,
{
    promise::spawn::spawn(async move { send_response(split_pane(split, client_id).await) })
        .detach();
}

async fn split_pane(split: SplitPane, client_id: Option<Arc<ClientId>>) -> anyhow::Result<Pdu> {
    let mux = Mux::get();
    let _identity = mux.with_identity(client_id);

    let (_pane_domain_id, window_id, tab_id) = mux
        .resolve_pane_id(split.pane_id)
        .ok_or_else(|| anyhow!("pane_id {} invalid", split.pane_id))?;

    let source = if let Some(move_pane_id) = split.move_pane_id {
        SplitSource::MovePane(move_pane_id)
    } else {
        SplitSource::Spawn {
            command: split.command,
            command_dir: split.command_dir,
        }
    };

    let (pane, size) = mux
        .split_pane(split.pane_id, split.split_request, source, split.domain)
        .await?;

    Ok::<Pdu, anyhow::Error>(Pdu::SpawnResponse(SpawnResponse {
        pane_id: pane.pane_id(),
        tab_id,
        window_id,
        size,
    }))
}

async fn domain_spawn_v2(spawn: SpawnV2, client_id: Option<Arc<ClientId>>) -> anyhow::Result<Pdu> {
    let mux = Mux::get();
    let _identity = mux.with_identity(client_id);

    let (tab, pane, window_id) = mux
        .spawn_tab_or_window(
            spawn.window_id,
            spawn.domain,
            spawn.command,
            spawn.command_dir,
            spawn.size,
            None, // optional current pane_id
            spawn.workspace,
            None, // optional gui window position
        )
        .await?;

    Ok::<Pdu, anyhow::Error>(Pdu::SpawnResponse(SpawnResponse {
        pane_id: pane.pane_id(),
        tab_id: tab.tab_id(),
        window_id,
        size: tab.get_size(),
    }))
}

fn schedule_move_pane<SND>(
    request: MovePaneToNewTab,
    send_response: SND,
    client_id: Option<Arc<ClientId>>,
) where
    SND: Fn(anyhow::Result<Pdu>) + 'static,
{
    promise::spawn::spawn(async move { send_response(move_pane(request, client_id).await) })
        .detach();
}

async fn move_pane(
    request: MovePaneToNewTab,
    client_id: Option<Arc<ClientId>>,
) -> anyhow::Result<Pdu> {
    let mux = Mux::get();
    let _identity = mux.with_identity(client_id);

    let (tab, window_id) = mux
        .move_pane_to_new_tab(
            request.pane_id,
            request.window_id,
            request.workspace_for_new_window,
        )
        .await?;

    Ok::<Pdu, anyhow::Error>(Pdu::MovePaneToNewTabResponse(MovePaneToNewTabResponse {
        tab_id: tab.tab_id(),
        window_id,
    }))
}

#[cfg(test)]
mod test {
    use super::*;
    use mux::output_signal::PaneOutputSignal;

    #[test]
    fn repaint_wait_without_signal_returns_at_once() {
        let start = Instant::now();
        let stopped = smol::block_on(wait_for_repaint(None, start, Duration::from_millis(50)));
        assert_eq!(stopped, ResizeRepaintStop::NoSignal);
        assert!(start.elapsed() < Duration::from_millis(40));
    }

    #[test]
    fn repaint_wait_stops_when_output_goes_quiet() {
        let signal = PaneOutputSignal::new();
        let mark = signal.begin_wait();
        let resized = Instant::now();
        // Play the parser thread: read a redraw shortly after the
        // resize, then apply it.
        let parser = Arc::clone(&signal);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_micros(500));
            parser.note_read();
            parser.note_idle(false, false);
        });
        let stopped = smol::block_on(wait_for_repaint(
            Some(mark),
            resized,
            Duration::from_millis(500),
        ));
        thread.join().unwrap();
        assert_eq!(stopped, ResizeRepaintStop::Quiet);
        assert!(resized.elapsed() < Duration::from_millis(400));
    }

    #[test]
    fn repaint_wait_does_not_block_the_calling_thread() {
        // Output that never finishes applying runs the wait to its
        // budget. Another task on the same executor thread must keep
        // running meanwhile, as the server's main thread serves other
        // clients during the wait.
        let signal = PaneOutputSignal::new();
        let mark = signal.begin_wait();
        signal.note_read();
        let resized = Instant::now();
        let ticks = std::sync::atomic::AtomicUsize::new(0);
        let (stopped, ()) = smol::block_on(async {
            futures::join!(
                wait_for_repaint(Some(mark), resized, Duration::from_millis(30)),
                async {
                    for _ in 0..5 {
                        smol::Timer::after(Duration::from_millis(1)).await;
                        ticks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            )
        });
        assert_eq!(stopped, ResizeRepaintStop::Budget);
        assert_eq!(ticks.load(std::sync::atomic::Ordering::SeqCst), 5);
    }

    #[test]
    fn only_one_push_is_queued_per_pane() {
        // A flooding pane notifies many times before its push task runs.
        // Only the first notification may queue a task; the rest are
        // covered by it. Once the task starts, the next output queues
        // a new one.
        let per_pane = Arc::new(Mutex::new(PerPane::default()));
        assert!(mark_push_scheduled(&per_pane));
        assert!(!mark_push_scheduled(&per_pane));
        assert!(!mark_push_scheduled(&per_pane));
        per_pane.lock().unwrap().push_scheduled = false;
        assert!(mark_push_scheduled(&per_pane));
    }

    #[test]
    fn resize_wait_guard_counts_waits() {
        let per_pane = Arc::new(Mutex::new(PerPane::default()));
        let first = ResizeRepaintWaitGuard::new(&per_pane);
        let second = ResizeRepaintWaitGuard::new(&per_pane);
        assert_eq!(per_pane.lock().unwrap().resize_repaint_waits, 2);
        drop(first);
        assert_eq!(per_pane.lock().unwrap().resize_repaint_waits, 1);
        drop(second);
        assert_eq!(per_pane.lock().unwrap().resize_repaint_waits, 0);
    }
}
