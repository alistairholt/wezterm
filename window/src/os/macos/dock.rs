//! The badge on WezTerm's Dock icon.

use super::nsstring;
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil};
use objc::*;

/// Show `label` as the Dock icon's badge, or clear it with `None`.
/// Main thread only.
pub fn set_dock_badge(label: Option<&str>) {
    unsafe {
        let tile: id = msg_send![NSApp(), dockTile];
        match label {
            Some(label) => {
                let label = nsstring(label);
                let _: () = msg_send![tile, setBadgeLabel: *label];
            }
            None => {
                let _: () = msg_send![tile, setBadgeLabel: nil];
            }
        }
    }
}
