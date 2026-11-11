use crate::domain::ClientInner;
use crate::pane::mousestate::MouseState;
use crate::pane::renderable::{hydrate_lines, RenderableInner, RenderableState};
use anyhow::bail;
use async_trait::async_trait;
use codec::*;
use config::configuration;
use config::keyassignment::ScrollbackEraseMode;
use mux::domain::DomainId;
use mux::pane::{
    alloc_pane_id, CachePolicy, CloseReason, ForEachPaneLogicalLine, LogicalLine, Pane, PaneId,
    Pattern, SearchResult, WithPaneLines,
};
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::TabId;
use mux::{Mux, MuxNotification};
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use rangeset::RangeSet;
use ratelim::RateLimiter;
use smol::channel::{Receiver, TryRecvError};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::input::KeyEvent;
use termwiz::surface::SequenceNo;
use url::Url;
use wezterm_dynamic::Value;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    Alert, Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, Progress, StableRowIndex,
    TerminalConfiguration, TerminalSize,
};

pub struct ClientPane {
    client: Arc<ClientInner>,
    local_pane_id: PaneId,
    pub remote_pane_id: PaneId,
    pub remote_tab_id: TabId,
    pub renderable: Mutex<RenderableState>,
    configured_palette: Mutex<ColorPalette>,
    palette: Mutex<ColorPalette>,
    application_palette: Mutex<bool>,
    writer: Mutex<PaneWriter>,
    mouse: Arc<Mutex<MouseState>>,
    clipboard: Mutex<Option<Arc<dyn Clipboard>>>,
    mouse_grabbed: Mutex<bool>,
    alt_screen_active: Mutex<bool>,
    ignore_next_kill: Mutex<bool>,
    user_vars: Mutex<HashMap<String, String>>,
    config: Mutex<Option<Arc<dyn TerminalConfiguration>>>,
    unseen_output: Mutex<bool>,
    progress: Mutex<Progress>,
    /// Set by `arm_live_resize_wait`; taken by the next resize.
    live_resize_wait_budget: Mutex<Option<Duration>>,
    /// The reply to the last resize that asked the server to wait.
    pending_resize_reply: Mutex<Option<PendingResizeReply>>,
}

/// How much longer than the server's wait budget the GUI waits for the
/// reply to a resize, to cover the round trip and the server's main
/// thread getting to it.
pub const RESIZE_REPLY_ALLOWANCE: Duration = Duration::from_millis(2);

/// The reply to a `Resize` that asked the server to wait for the pane to
/// redraw. Wait on it with `wait`; if it is dropped instead, the reply is
/// applied on the main thread when it arrives. Either way the response
/// channel stays open until the reply comes, because the client thread
/// treats a closed channel as the client going away.
pub struct PendingResizeReply {
    rx: Option<Receiver<anyhow::Result<Pdu>>>,
    local_pane_id: PaneId,
}

/// What happened to a resize reply that the GUI waited for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeReplyOutcome {
    /// The reply arrived and its changes were applied to the line cache.
    Applied {
        stopped: ResizeRepaintStop,
        server_waited: Duration,
        lines: usize,
    },
    /// The reply arrived but refers to images that must be fetched first;
    /// it is applied once they arrive.
    Deferred {
        stopped: ResizeRepaintStop,
        server_waited: Duration,
    },
    /// Nothing arrived by the deadline; the reply is applied when it does.
    Timeout,
    /// The request failed, or the pane has gone.
    Failed,
}

impl PendingResizeReply {
    /// Block until the reply arrives or `deadline` passes. When it arrives
    /// in time, its changes are applied before this returns, so the next
    /// paint shows them. Call on the main thread.
    pub fn wait(mut self, deadline: Instant) -> ResizeReplyOutcome {
        let Some(rx) = self.rx.take() else {
            return ResizeReplyOutcome::Failed;
        };
        let reply = match rx.try_recv() {
            Ok(reply) => Some(reply),
            Err(TryRecvError::Closed) => Some(Err(anyhow::anyhow!("client went away"))),
            Err(TryRecvError::Empty) => smol::block_on(smol::future::or(
                async {
                    Some(
                        rx.recv()
                            .await
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("client went away"))),
                    )
                },
                async {
                    smol::Timer::at(deadline).await;
                    None
                },
            )),
        };
        match reply {
            Some(reply) => apply_resize_reply(self.local_pane_id, reply),
            None => {
                // Drop collects it later.
                self.rx.replace(rx);
                ResizeReplyOutcome::Timeout
            }
        }
    }
}

impl Drop for PendingResizeReply {
    fn drop(&mut self) {
        let Some(rx) = self.rx.take() else {
            return;
        };
        let local_pane_id = self.local_pane_id;
        promise::spawn::spawn_into_main_thread(async move {
            let reply = rx
                .recv()
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("client went away")));
            apply_resize_reply(local_pane_id, reply);
        })
        .detach();
    }
}

/// Apply the reply to a `Resize`. Call on the main thread.
fn apply_resize_reply(local_pane_id: PaneId, reply: anyhow::Result<Pdu>) -> ResizeReplyOutcome {
    let resp = match reply {
        Ok(Pdu::ResizeRepaintResponse(resp)) => resp,
        // A resize that didn't ask for a wait.
        Ok(Pdu::UnitResponse(_)) => {
            return ResizeReplyOutcome::Applied {
                stopped: ResizeRepaintStop::NoSignal,
                server_waited: Duration::ZERO,
                lines: 0,
            }
        }
        Ok(Pdu::ErrorResponse(ErrorResponse { reason })) => {
            log::error!("resize pane {local_pane_id}: {reason}");
            return ResizeReplyOutcome::Failed;
        }
        Ok(other) => {
            log::error!("resize pane {local_pane_id}: unexpected response {other:?}");
            return ResizeReplyOutcome::Failed;
        }
        Err(err) => {
            log::error!("resize pane {local_pane_id}: {err:#}");
            return ResizeReplyOutcome::Failed;
        }
    };
    let stopped = resp.stopped;
    let server_waited = Duration::from_micros(resp.waited_us);
    let Some(mut changes) = resp.changes else {
        return ResizeReplyOutcome::Applied {
            stopped,
            server_waited,
            lines: 0,
        };
    };
    let Some(pane) = Mux::get().get_pane(local_pane_id) else {
        return ResizeReplyOutcome::Failed;
    };
    if pane.downcast_ref::<ClientPane>().is_none() {
        return ResizeReplyOutcome::Failed;
    }

    if changes.bonus_lines.has_images() {
        promise::spawn::spawn(async move {
            if let Some(client_pane) = pane.downcast_ref::<ClientPane>() {
                client_pane.apply_render_changes(changes, true).await;
            }
        })
        .detach();
        return ResizeReplyOutcome::Deferred {
            stopped,
            server_waited,
        };
    }

    let client_pane = pane.downcast_ref::<ClientPane>().expect("checked above");
    let (bonus_lines, _no_images) = std::mem::take(&mut changes.bonus_lines).extract_data();
    let lines = bonus_lines.len();
    let applied = client_pane.apply_hydrated_render_changes(changes, bonus_lines, true);
    ResizeReplyOutcome::Applied {
        stopped,
        server_waited,
        // Zero if a newer update had already been applied.
        lines: if applied { lines } else { 0 },
    }
}

impl ClientPane {
    pub fn new(
        client: &Arc<ClientInner>,
        remote_tab_id: TabId,
        remote_pane_id: PaneId,
        size: TerminalSize,
        title: &str,
    ) -> Self {
        let local_pane_id = alloc_pane_id();
        let writer = PaneWriter {
            client: Arc::clone(client),
            remote_pane_id,
        };

        let mouse = Arc::new(Mutex::new(MouseState::new(
            remote_pane_id,
            client.client.clone(),
        )));

        let fetch_limiter =
            RateLimiter::new(|config| config.ratelimit_mux_line_prefetches_per_second);

        let render = RenderableState {
            inner: RefCell::new(RenderableInner::new(
                client,
                remote_pane_id,
                local_pane_id,
                RenderableDimensions {
                    cols: size.cols as _,
                    viewport_rows: size.rows as _,
                    scrollback_rows: size.rows as _,
                    physical_top: 0,
                    scrollback_top: 0,
                    dpi: size.dpi,
                    pixel_width: size.pixel_width,
                    pixel_height: size.pixel_height,
                    reverse_video: false,
                },
                title,
                fetch_limiter,
            )),
        };

        let config = configuration();
        let palette: ColorPalette = config.resolved_palette.clone().into();

        // Advise the server of our palette preference
        promise::spawn::spawn({
            let palette = palette.clone();
            let client = Arc::clone(client);
            async move {
                client
                    .client
                    .set_configured_palette_for_pane(SetPalette {
                        pane_id: remote_pane_id,
                        palette,
                    })
                    .await
            }
        })
        .detach();

        Self {
            client: Arc::clone(client),
            mouse,
            remote_pane_id,
            local_pane_id,
            remote_tab_id,
            application_palette: Mutex::new(false),
            renderable: Mutex::new(render),
            writer: Mutex::new(writer),
            configured_palette: Mutex::new(palette.clone()),
            palette: Mutex::new(palette),
            clipboard: Mutex::new(None),
            mouse_grabbed: Mutex::new(false),
            alt_screen_active: Mutex::new(false),
            ignore_next_kill: Mutex::new(false),
            unseen_output: Mutex::new(false),
            user_vars: Mutex::new(HashMap::new()),
            config: Mutex::new(None),
            progress: Mutex::new(Progress::default()),
            live_resize_wait_budget: Mutex::new(None),
            pending_resize_reply: Mutex::new(None),
        }
    }

    pub async fn process_unilateral(&self, pdu: Pdu) -> anyhow::Result<()> {
        match pdu {
            Pdu::GetPaneRenderChangesResponse(delta) => {
                self.apply_render_changes(delta, false).await;
            }
            Pdu::SetClipboard(SetClipboard {
                clipboard,
                selection,
                ..
            }) => match self.clipboard.lock().as_ref() {
                Some(clip) => {
                    log::debug!(
                        "Pdu::SetClipboard pane={} remote={} {:?} {:?}",
                        self.local_pane_id,
                        self.remote_pane_id,
                        selection,
                        clipboard
                    );
                    clip.set_contents(selection, clipboard)?;
                }
                None => {
                    log::error!("ClientPane: Ignoring SetClipboard request {:?}", clipboard);
                }
            },
            Pdu::SetPalette(SetPalette { palette, .. }) => {
                *self.application_palette.lock() = palette != *self.configured_palette.lock();

                *self.palette.lock() = palette;
                let mux = Mux::get();
                self.renderable.lock().inner.borrow_mut().make_all_stale();
                mux.notify(MuxNotification::Alert {
                    pane_id: self.local_pane_id,
                    alert: Alert::PaletteChanged,
                });
            }
            Pdu::NotifyAlert(NotifyAlert { alert, .. }) => {
                let mux = Mux::get();
                match &alert {
                    Alert::SetUserVar { name, value } => {
                        self.user_vars.lock().insert(name.clone(), value.clone());
                    }
                    Alert::OutputSinceFocusLost => {
                        *self.unseen_output.lock() = true;
                        mux.notify(MuxNotification::Alert {
                            pane_id: self.local_pane_id,
                            alert: Alert::OutputSinceFocusLost,
                        });
                    }
                    Alert::Progress(progress) => {
                        *self.progress.lock() = progress.clone();
                        mux.notify(MuxNotification::Alert {
                            pane_id: self.local_pane_id,
                            alert: Alert::Progress(progress.clone()),
                        });
                    }
                    _ => {}
                }
                mux.notify(MuxNotification::Alert {
                    pane_id: self.local_pane_id,
                    alert,
                });
            }
            Pdu::PaneRemoved(PaneRemoved { pane_id }) => {
                log::trace!("remote pane {} has been removed", pane_id);
                self.renderable.lock().inner.borrow_mut().dead = true;
                let mux = Mux::get();
                mux.prune_dead_windows();

                self.client.expire_stale_mappings();
            }
            Pdu::PaneFocused(PaneFocused { pane_id }) => {
                // We get here whenever the pane focus is changed on the
                // server. That might be due to the user here in the GUI
                // doing things, or it may be due to a "remote"
                // `wezterm cli activate-pane-direction` or similar call
                // from some other actor.
                // The latter case is the important one: it is desirable
                // for the focus change to be reflected locally after it
                // has been changed on the server, so we work to apply
                // it here.
                log::trace!("advised of remote pane focus: {pane_id}");

                let mux = Mux::get();
                if let Err(err) = mux.focus_pane_and_containing_tab(self.local_pane_id) {
                    log::error!("Error reconciling remote PaneFocused notification: {err:#}");
                }
            }
            _ => bail!("unhandled unilateral pdu: {:?}", pdu),
        };
        Ok(())
    }

    /// Fetch any images the changes refer to, then apply them.
    async fn apply_render_changes(
        &self,
        mut delta: GetPaneRenderChangesResponse,
        from_resize_reply: bool,
    ) {
        let bonus_lines = std::mem::take(&mut delta.bonus_lines);
        let client = { Arc::clone(&self.renderable.lock().inner.borrow().client) };
        let bonus_lines = hydrate_lines(client, delta.pane_id, bonus_lines).await;
        self.apply_hydrated_render_changes(delta, bonus_lines, from_resize_reply);
    }

    /// Apply changes whose lines are ready to use. Returns false if the
    /// changes were skipped because a resize reply already applied a
    /// newer state.
    fn apply_hydrated_render_changes(
        &self,
        delta: GetPaneRenderChangesResponse,
        bonus_lines: Vec<(StableRowIndex, Line)>,
        from_resize_reply: bool,
    ) -> bool {
        let mouse_grabbed = delta.mouse_grabbed;
        let alt_screen_active = delta.alt_screen_active;
        {
            let render = self.renderable.lock();
            let mut inner = render.inner.borrow_mut();
            if !from_resize_reply && delta.seqno < inner.resize_reply_seqno {
                // The resize reply was applied straight away on the main
                // thread, ahead of this update which was queued earlier.
                // Applying it now would put back older lines and the old
                // dimensions.
                log::trace!(
                    "pane {} skipping changes at seqno {} older than resize reply at {}",
                    self.local_pane_id,
                    delta.seqno,
                    inner.resize_reply_seqno
                );
                return false;
            }
            if from_resize_reply && delta.seqno < inner.seqno {
                // A reply that nobody waited for, overtaken by a newer
                // update. The resize already marked every cached line
                // stale, so lines it would have refreshed are fetched
                // again when next drawn.
                log::trace!(
                    "pane {} skipping resize reply at seqno {} older than {}",
                    self.local_pane_id,
                    delta.seqno,
                    inner.seqno
                );
                return false;
            }
            if from_resize_reply {
                inner.resize_reply_seqno = inner.resize_reply_seqno.max(delta.seqno);
            }
            inner.apply_changes_to_surface(delta, bonus_lines);
        }
        *self.mouse_grabbed.lock() = mouse_grabbed;
        *self.alt_screen_active.lock() = alt_screen_active;
        true
    }

    /// Ask that the next resize of this pane waits, on the server, up to
    /// `budget` for the program to redraw, and returns the result. Use
    /// `take_live_resize_reply` after resizing to collect the reply.
    /// Returns false, and does nothing, unless the pane is in a unix
    /// domain: over a network the round trip alone would use up the
    /// budget on every step.
    pub fn arm_live_resize_wait(&self, budget: Duration) -> bool {
        if !self.client.client.is_local {
            return false;
        }
        self.live_resize_wait_budget.lock().replace(budget);
        true
    }

    /// Returns the reply to a resize sent after `arm_live_resize_wait`, if
    /// one was sent, and disarms the wait. Dropping the result without
    /// waiting on it applies the reply whenever it arrives.
    pub fn take_live_resize_reply(&self) -> Option<PendingResizeReply> {
        self.live_resize_wait_budget.lock().take();
        self.pending_resize_reply.lock().take()
    }

    pub fn remote_pane_id(&self) -> TabId {
        self.remote_pane_id
    }

    /// Arrange to suppress the next Pane::kill call.
    /// This is a bit of a hack that we use when closing a window;
    /// our Domain::local_window_is_closing impl calls this for each
    /// ClientPane in the window so that closing a window effectively
    /// "detaches" the window so that reconnecting later will resume
    /// from where they left off.
    /// It isn't perfect.
    pub fn ignore_next_kill(&self) {
        *self.ignore_next_kill.lock() = true;
    }
}

#[async_trait(?Send)]
impl Pane for ClientPane {
    fn pane_id(&self) -> PaneId {
        self.local_pane_id
    }

    fn get_metadata(&self) -> Value {
        let renderable = self.renderable.lock();
        let inner = renderable.inner.borrow();

        let mut map: BTreeMap<Value, Value> = BTreeMap::new();
        map.insert(
            Value::String("is_tardy".to_string()),
            Value::Bool(inner.is_tardy()),
        );
        map.insert(
            Value::String("since_last_response_ms".to_string()),
            Value::U64(inner.last_recv_time.elapsed().as_millis() as u64),
        );

        Value::Object(map.into())
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        self.renderable.lock().get_cursor_position()
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        self.renderable.lock().get_dimensions()
    }

    fn with_lines_mut(&self, lines: Range<StableRowIndex>, with_lines: &mut dyn WithPaneLines) {
        mux::pane::impl_with_lines_via_get_lines(self, lines, with_lines);
    }

    fn for_each_logical_line_in_stable_range_mut(
        &self,
        lines: Range<StableRowIndex>,
        for_line: &mut dyn ForEachPaneLogicalLine,
    ) {
        mux::pane::impl_for_each_logical_line_via_get_logical_lines(self, lines, for_line);
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        self.renderable.lock().get_lines(lines)
    }

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        mux::pane::impl_get_logical_lines_via_get_lines(self, lines)
    }

    fn get_current_seqno(&self) -> SequenceNo {
        self.renderable.lock().get_current_seqno()
    }

    fn get_changed_since(
        &self,
        lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> RangeSet<StableRowIndex> {
        self.renderable.lock().get_changed_since(lines, seqno)
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.lock().replace(Arc::clone(clipboard));
    }

    fn get_title(&self) -> String {
        let renderable = self.renderable.lock();
        let inner = renderable.inner.borrow();
        inner.title.clone()
    }

    fn get_progress(&self) -> Progress {
        self.progress.lock().clone()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        self.renderable
            .lock()
            .inner
            .borrow_mut()
            .predict_from_paste(text);

        let data = text.to_owned();
        promise::spawn::spawn(async move {
            client
                .client
                .send_paste(SendPaste {
                    pane_id: remote_pane_id,
                    data,
                })
                .await
        })
        .detach();
        self.renderable.lock().inner.borrow_mut().update_last_send();
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(None)
    }

    fn writer(&self) -> MappedMutexGuard<'_, dyn std::io::Write> {
        MutexGuard::map(self.writer.lock(), |writer| {
            let w: &mut dyn std::io::Write = writer;
            w
        })
    }

    fn set_zoomed(&self, zoomed: bool) {
        let render = self.renderable.lock();
        let mut inner = render.inner.borrow_mut();
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        let remote_tab_id = self.remote_tab_id;
        // Invalidate any cached rows on a resize
        inner.make_all_stale();
        promise::spawn::spawn(async move {
            client
                .client
                .set_zoomed(SetPaneZoomed {
                    containing_tab_id: remote_tab_id,
                    pane_id: remote_pane_id,
                    zoomed,
                })
                .await
        })
        .detach();
        inner.update_last_send();
    }

    fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
        let render = self.renderable.lock();
        let mut inner = render.inner.borrow_mut();

        let cols = size.cols as usize;
        let rows = size.rows as usize;

        if inner.dimensions.cols != cols
            || inner.dimensions.viewport_rows != rows
            || inner.dimensions.pixel_width != size.pixel_width
            || inner.dimensions.pixel_height != size.pixel_height
        {
            inner.dimensions.cols = cols;
            inner.dimensions.viewport_rows = rows;
            inner.dimensions.pixel_width = size.pixel_width;
            inner.dimensions.pixel_height = size.pixel_height;

            // Invalidate any cached rows on a resize
            inner.make_all_stale();

            // Send the request now rather than from a spawned task, so
            // that resizes reach the server in order and the GUI can wait
            // for the reply before it paints.
            let wait_budget = self.live_resize_wait_budget.lock().take();
            let repaint_wait_ms = wait_budget
                .map(|budget| (budget.as_millis() as u64).max(1))
                .unwrap_or(0);
            let pdu = Pdu::Resize(Resize {
                containing_tab_id: self.remote_tab_id,
                pane_id: self.remote_pane_id,
                size,
                repaint_wait_ms,
            });
            match self.client.client.send_pdu_now(pdu) {
                Ok(rx) => {
                    let reply = PendingResizeReply {
                        rx: Some(rx),
                        local_pane_id: self.local_pane_id,
                    };
                    if repaint_wait_ms > 0 {
                        log::trace!(
                            "live resize repaint wait: pane {} sent resize {}x{} \
                             to remote pane {}, server budget {}ms",
                            self.local_pane_id,
                            size.cols,
                            size.rows,
                            self.remote_pane_id,
                            repaint_wait_ms
                        );
                        // An older reply that nobody waited for is applied
                        // when it arrives.
                        self.pending_resize_reply.lock().replace(reply);
                    }
                    // Otherwise dropping `reply` collects the response
                    // in the background.
                }
                Err(err) => {
                    log::error!("resize pane {}: {err:#}", self.local_pane_id);
                }
            }
            inner.update_last_send();
        }
        Ok(())
    }

    async fn search(
        &self,
        pattern: Pattern,
        range: Range<StableRowIndex>,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        match self
            .client
            .client
            .search_scrollback(SearchScrollbackRequest {
                pane_id: self.remote_pane_id,
                pattern,
                range,
                limit,
            })
            .await
        {
            Ok(SearchScrollbackResponse { results }) => Ok(results),
            Err(e) => Err(e),
        }
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        let input_serial;
        {
            let renderable = self.renderable.lock();
            let mut inner = renderable.inner.borrow_mut();
            inner.input_serial = InputSerial::now();
            input_serial = inner.input_serial;
            inner.predict_from_key_event(key, mods);
        }
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        promise::spawn::spawn(async move {
            client
                .client
                .key_down(SendKeyDown {
                    pane_id: remote_pane_id,
                    event: KeyEvent {
                        key,
                        modifiers: mods,
                    },
                    input_serial,
                })
                .await
        })
        .detach();
        self.renderable.lock().inner.borrow_mut().update_last_send();
        Ok(())
    }

    fn key_up(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
        // TODO: decide how to handle key_up for mux client
        Ok(())
    }

    fn kill(&self) {
        let mut ignore = self.ignore_next_kill.lock();
        if *ignore {
            *ignore = false;
            return;
        }
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        let local_domain_id = self.client.local_domain_id;

        // We only want to ask the server to kill the pane if the user
        // explicitly requested it to die.
        // Domain detaching can implicitly call Pane::kill on the panes
        // in the domain, so we need to check here whether the domain is
        // in the detached state; if so then we must skip sending the
        // kill to the server.
        let mut send_kill = true;

        {
            let mux = Mux::get();
            if let Some(client_domain) = mux.get_domain(local_domain_id) {
                if client_domain.state() == mux::domain::DomainState::Detached {
                    send_kill = false;
                }
            }
        }

        if send_kill {
            promise::spawn::spawn(async move {
                client
                    .client
                    .kill_pane(KillPane {
                        pane_id: remote_pane_id,
                    })
                    .await
            })
            .detach();
        }
    }

    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()> {
        self.mouse.lock().append(event);
        if MouseState::next(Arc::clone(&self.mouse)) {
            self.renderable.lock().inner.borrow_mut().update_last_send();
        }
        Ok(())
    }

    fn is_dead(&self) -> bool {
        self.renderable.lock().inner.borrow().dead
    }

    fn palette(&self) -> ColorPalette {
        self.palette.lock().clone()
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        *self.mouse_grabbed.lock()
    }

    fn is_alt_screen_active(&self) -> bool {
        *self.alt_screen_active.lock()
    }

    fn get_current_working_dir(&self, _policy: CachePolicy) -> Option<Url> {
        self.renderable.lock().inner.borrow().working_dir.clone()
    }

    fn focus_changed(&self, focused: bool) {
        if focused {
            self.advise_focus();
            *self.unseen_output.lock() = false;
        }
    }

    fn erase_scrollback(&self, erase_mode: ScrollbackEraseMode) {
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        promise::spawn::spawn(async move {
            client
                .client
                .erase_scrollback(EraseScrollbackRequest {
                    pane_id: remote_pane_id,
                    erase_mode,
                })
                .await
        })
        .detach();
    }

    fn advise_focus(&self) {
        let mut focused_pane = self.client.focused_remote_pane_id.lock().unwrap();
        if *focused_pane != Some(self.remote_pane_id) {
            focused_pane.replace(self.remote_pane_id);
            let client = Arc::clone(&self.client);
            let remote_pane_id = self.remote_pane_id;
            promise::spawn::spawn(async move {
                client
                    .client
                    .set_focused_pane_id(SetFocusedPane {
                        pane_id: remote_pane_id,
                    })
                    .await
            })
            .detach();
        }
    }

    fn has_unseen_output(&self) -> bool {
        *self.unseen_output.lock()
    }

    fn can_close_without_prompting(&self, reason: CloseReason) -> bool {
        match reason {
            CloseReason::Window => true,
            CloseReason::Tab => false,
            CloseReason::Pane => false,
        }
    }

    fn copy_user_vars(&self) -> HashMap<String, String> {
        self.user_vars.lock().clone()
    }

    fn set_config(&self, config: Arc<dyn TerminalConfiguration>) {
        let palette = config.color_palette();
        // If the application running in the pane hasn't changed the
        // palette through escape sequences, speculatively adopt the
        // new palette so that it updates with the lowest latency.
        if !*self.application_palette.lock() {
            *self.palette.lock() = palette.clone();
        }
        *self.configured_palette.lock() = palette.clone();

        // and now send the color palette to the server
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        promise::spawn::spawn(async move {
            client
                .client
                .set_configured_palette_for_pane(SetPalette {
                    pane_id: remote_pane_id,
                    palette,
                })
                .await
        })
        .detach();
        self.config.lock().replace(config);
    }

    fn get_config(&self) -> Option<Arc<dyn TerminalConfiguration>> {
        self.config.lock().clone()
    }
}

struct PaneWriter {
    client: Arc<ClientInner>,
    remote_pane_id: TabId,
}

impl std::io::Write for PaneWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        promise::spawn::block_on(self.client.client.write_to_pane(WriteToPane {
            pane_id: self.remote_pane_id,
            data: data.to_vec(),
        }))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
