//! A Save panel, shown as a sheet on the key window.

use super::{nsstring, nsstring_to_str};
use block::ConcreteBlock;
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil};
use cocoa::foundation::NSInteger;
use objc::*;
use std::cell::RefCell;
use std::path::PathBuf;

const NS_MODAL_RESPONSE_OK: NSInteger = 1;

/// Ask where to save a file, starting with `default_name`, and call
/// `done` with the chosen path, or `None` if the user cancelled.
///
/// The panel is a sheet on the key window, and `done` runs later from
/// the sheet's completion handler. A modal panel would run a nested
/// event loop while the caller is still in the middle of handling an
/// event. Call it from a fresh task, not while handling a window
/// event: attaching the sheet calls back into the window. Main thread
/// only.
pub fn choose_save_path(default_name: &str, done: impl FnOnce(Option<PathBuf>) + 'static) {
    unsafe {
        let panel: id = msg_send![class!(NSSavePanel), savePanel];
        let name = nsstring(default_name);
        let _: () = msg_send![panel, setNameFieldStringValue: *name];
        let _: () = msg_send![panel, setCanCreateDirectories: true];

        let panel_ptr = panel;
        // The block must be Fn; it runs once, so take `done` out.
        let done = RefCell::new(Some(done));
        let handler = ConcreteBlock::new(move |response: NSInteger| {
            let Some(done) = done.borrow_mut().take() else {
                return;
            };
            if response != NS_MODAL_RESPONSE_OK {
                done(None);
                return;
            }
            let url: id = msg_send![panel_ptr, URL];
            if url == nil {
                done(None);
                return;
            }
            let path: id = msg_send![url, path];
            done(Some(PathBuf::from(nsstring_to_str(path))));
        });
        let handler = handler.copy();

        let window: id = msg_send![NSApp(), keyWindow];
        if window == nil {
            let response: NSInteger = msg_send![panel, runModal];
            (*handler).call((response,));
        } else {
            let _: () =
                msg_send![panel, beginSheetModalForWindow: window completionHandler: &*handler];
        }
    }
}
