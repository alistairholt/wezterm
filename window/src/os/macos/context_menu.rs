//! A right-click context menu.

use super::menu::{Menu, MenuItem, RepresentedItem};
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil};
use cocoa::foundation::NSPoint;
use config::keyassignment::KeyAssignment;
use objc::*;
use std::cell::Cell;

/// One entry in a context menu.
pub enum ContextMenuItem {
    Action {
        title: String,
        action: KeyAssignment,
        enabled: bool,
    },
    Separator,
}

/// Shows `items` as a context menu. `at` is a point in the key window's
/// content, in points from its top left corner; with `None` the menu
/// opens at the mouse pointer. Choosing an item sends its action to the
/// key window, the same way the menu bar does.
///
/// The menu runs a nested event loop until it closes, so call this from
/// a fresh task, not while handling a window event: the chosen action
/// is dispatched back into the window. Main thread only.
pub fn show_context_menu(items: Vec<ContextMenuItem>, at: Option<(f64, f64)>) {
    // Don't open a second menu from inside the first one's event loop.
    if MENU_OPEN.with(|open| open.replace(true)) {
        return;
    }
    show(items, at);
    MENU_OPEN.with(|open| open.set(false));
}

thread_local! {
    static MENU_OPEN: Cell<bool> = Cell::new(false);
}

fn show(items: Vec<ContextMenuItem>, at: Option<(f64, f64)>) {
    let menu = Menu::new_with_title("");
    for item in items {
        match item {
            ContextMenuItem::Separator => menu.add_item(&MenuItem::new_separator()),
            ContextMenuItem::Action {
                title,
                action,
                enabled,
            } => {
                let menu_item =
                    MenuItem::new_with(&title, Some(sel!(weztermPerformKeyAssignment:)), "");
                menu_item.set_represented_item(RepresentedItem::KeyAssignment(action));
                menu_item.set_enabled(enabled);
                menu.add_item(&menu_item);
            }
        }
    }
    unsafe {
        let ns_menu = menu.as_id();
        // Use each item's own enabled state rather than asking the
        // responder chain.
        let () = msg_send![ns_menu, setAutoenablesItems: false];
        let window: id = msg_send![NSApp(), keyWindow];
        let view: id = if window == nil {
            nil
        } else {
            msg_send![window, contentView]
        };
        match at {
            // WezTerm's view is flipped: y grows downwards from the top.
            Some((x, y)) if view != nil => {
                let location = NSPoint::new(x, y);
                let _: bool = msg_send![ns_menu, popUpMenuPositioningItem: nil atLocation: location inView: view];
            }
            _ => {
                // With no view the location is in screen coordinates.
                let location: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                let _: bool = msg_send![ns_menu, popUpMenuPositioningItem: nil atLocation: location inView: nil as id];
            }
        }
    }
}
