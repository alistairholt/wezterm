use crate::overlay::{confirm, start_overlay_pane};
use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ClipboardPasteSource};
use mux::pane::Pane;
use mux::Mux;
use std::sync::Arc;
use window::{Clipboard, WindowOps};

impl TermWindow {
    /// Paste `text` into `pane`, and into any panes Broadcast Input
    /// copies it to. A program that hasn't turned on bracketed paste
    /// runs each pasted line as soon as it arrives, so when the text
    /// has a line break, ask first (`confirm_multiline_paste`).
    pub fn paste_with_confirmation(&mut self, pane: &Arc<dyn Pane>, text: String) {
        let pane_id = pane.pane_id();
        let needs_confirmation = self.config.confirm_multiline_paste
            && text.contains(['\n', '\r'])
            && !pane.bracketed_paste_enabled()
            && self.pane_state(pane_id).overlay.is_none();

        if !needs_confirmation {
            pane.send_paste(&text).ok();
            for target in self.broadcast_input_targets(pane) {
                target.send_paste(&text).ok();
            }
            return;
        }

        let lines = text.lines().count().max(1);
        let message = format!(
            "📋 Paste {lines} line{}? The program in this pane hasn't turned \
             on bracketed paste, so each line will run as soon as it arrives.",
            if lines == 1 { "" } else { "s" }
        );
        let window = self.window.clone().unwrap();
        let (overlay, future) = start_overlay_pane(self, pane, move |pane_id, mut term| {
            let confirmed = confirm::run_confirmation(&message, &mut term)?;
            TermWindow::schedule_cancel_overlay_for_pane(window.clone(), pane_id);
            if confirmed {
                window.notify(TermWindowNotif::Apply(Box::new(move |myself| {
                    if let Some(pane) = Mux::get().get_pane(pane_id) {
                        pane.send_paste(&text).ok();
                        for target in myself.broadcast_input_targets(&pane) {
                            target.send_paste(&text).ok();
                        }
                    }
                })));
            }
            Ok(())
        });
        self.assign_overlay_for_pane(pane_id, overlay);
        promise::spawn::spawn(future).detach();
    }

    pub fn copy_to_clipboard(&self, clipboard: ClipboardCopyDestination, text: String) {
        let clipboard = match clipboard {
            ClipboardCopyDestination::Clipboard => [Some(Clipboard::Clipboard), None],
            ClipboardCopyDestination::PrimarySelection => [Some(Clipboard::PrimarySelection), None],
            ClipboardCopyDestination::ClipboardAndPrimarySelection => [
                Some(Clipboard::Clipboard),
                Some(Clipboard::PrimarySelection),
            ],
        };
        for &c in &clipboard {
            if let Some(c) = c {
                self.window.as_ref().unwrap().set_clipboard(c, text.clone());
            }
        }
    }

    pub fn paste_from_clipboard(&mut self, pane: &Arc<dyn Pane>, clipboard: ClipboardPasteSource) {
        let pane_id = pane.pane_id();
        log::trace!(
            "paste_from_clipboard in pane {} {:?}",
            pane.pane_id(),
            clipboard
        );
        let window = self.window.as_ref().unwrap().clone();
        let clipboard = match clipboard {
            ClipboardPasteSource::Clipboard => Clipboard::Clipboard,
            ClipboardPasteSource::PrimarySelection => Clipboard::PrimarySelection,
        };
        let future = window.get_clipboard(clipboard);
        promise::spawn::spawn(async move {
            if let Ok(clip) = future.await {
                window.notify(TermWindowNotif::Apply(Box::new(move |myself| {
                    let pane = myself
                        .pane_state(pane_id)
                        .overlay
                        .as_ref()
                        .map(|overlay| overlay.pane.clone())
                        .or_else(|| {
                            let mux = Mux::get();
                            mux.get_pane(pane_id)
                        });
                    if let Some(pane) = pane {
                        myself.paste_with_confirmation(&pane, clip);
                    }
                })));
            }
        })
        .detach();
        self.maybe_scroll_to_bottom_for_input(&pane);
    }
}
