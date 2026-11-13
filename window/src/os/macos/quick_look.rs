//! The Quick Look panel, as Finder shows for a file when Space is
//! pressed.

use super::nsstring;
use cocoa::base::{id, nil, BOOL, YES};
use cocoa::foundation::NSInteger;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Sel};
use objc::*;
use std::cell::RefCell;
use std::path::PathBuf;

// QLPreviewPanel lives in QuickLookUI, which is part of Quartz.
#[link(name = "Quartz", kind = "framework")]
extern "C" {}

const SOURCE_CLS_NAME: &str = "WezTermQuickLookSource";

thread_local! {
    /// The files on show: an NSArray of NSURL. NSURL is itself a
    /// preview item, so the panel takes them as they are.
    static ITEMS: RefCell<Option<StrongPtr>> = RefCell::new(None);
    /// The object the panel asks for its items
    static SOURCE: RefCell<Option<StrongPtr>> = RefCell::new(None);
}

fn items() -> id {
    ITEMS.with(|items| items.borrow().as_ref().map_or(nil, |items| **items))
}

extern "C" fn number_of_items(_: &Object, _: Sel, _panel: id) -> NSInteger {
    let items = items();
    if items == nil {
        return 0;
    }
    unsafe {
        let count: NSInteger = msg_send![items, count];
        count
    }
}

extern "C" fn item_at_index(_: &Object, _: Sel, _panel: id, index: NSInteger) -> id {
    let items = items();
    if items == nil {
        return nil;
    }
    unsafe {
        let count: NSInteger = msg_send![items, count];
        if index < 0 || index >= count {
            return nil;
        }
        msg_send![items, objectAtIndex: index]
    }
}

fn source() -> id {
    SOURCE.with(|source| {
        let mut source = source.borrow_mut();
        if source.is_none() {
            let cls = Class::get(SOURCE_CLS_NAME).unwrap_or_else(|| {
                let mut cls = ClassDecl::new(SOURCE_CLS_NAME, class!(NSObject))
                    .expect("Unable to register the Quick Look source class");
                unsafe {
                    cls.add_method(
                        sel!(numberOfPreviewItemsInPreviewPanel:),
                        number_of_items as extern "C" fn(&Object, Sel, id) -> NSInteger,
                    );
                    cls.add_method(
                        sel!(previewPanel:previewItemAtIndex:),
                        item_at_index as extern "C" fn(&Object, Sel, id, NSInteger) -> id,
                    );
                }
                cls.register()
            });
            unsafe {
                source.replace(StrongPtr::new(msg_send![cls, new]));
            }
        }
        **source.as_ref().unwrap()
    })
}

fn panel() -> Option<id> {
    let cls = Class::get("QLPreviewPanel")?;
    let panel: id = unsafe { msg_send![cls, sharedPreviewPanel] };
    (panel != nil).then_some(panel)
}

/// Shows `paths` in the Quick Look panel, the first one in front. The
/// arrow keys step through the rest. If the panel is already open it
/// switches to these files.
///
/// Showing the panel changes the key window, which calls back into the
/// window that had it. Call this from a fresh task, not while handling
/// a window event. Main thread only.
pub fn show(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    unsafe {
        let array: id = msg_send![class!(NSMutableArray), arrayWithCapacity: paths.len()];
        for path in paths {
            let Some(path) = path.to_str() else {
                continue;
            };
            let url: id = msg_send![class!(NSURL), fileURLWithPath: *nsstring(path)];
            if url != nil {
                let _: () = msg_send![array, addObject: url];
            }
        }
        ITEMS.with(|items| items.borrow_mut().replace(StrongPtr::retain(array)));

        let Some(panel) = panel() else {
            log::error!("Quick Look is not available");
            return;
        };
        let visible: BOOL = msg_send![panel, isVisible];
        if visible == YES {
            // The panel may be under another window's control, or ours
            // from an earlier click; either way, look again.
            let _: () = msg_send![panel, updateController];
            let _: () = msg_send![panel, reloadData];
            let _: () = msg_send![panel, setCurrentPreviewItemIndex: 0 as NSInteger];
        }
        // The panel now asks the key window's views whether one of them
        // will control it; see `accepts_control`.
        let _: () = msg_send![panel, makeKeyAndOrderFront: nil];
    }
}

/// For a view's `acceptsPreviewPanelControl:`: true if there are files
/// to show.
pub fn accepts_control() -> bool {
    items() != nil
}

/// For a view's `beginPreviewPanelControl:`.
pub fn begin_control(panel: id) {
    unsafe {
        let _: () = msg_send![panel, setDataSource: source()];
        let _: () = msg_send![panel, reloadData];
        let _: () = msg_send![panel, setCurrentPreviewItemIndex: 0 as NSInteger];
    }
}

/// For a view's `endPreviewPanelControl:`. The panel is closing or
/// another window has taken it over.
pub fn end_control(panel: id) {
    unsafe {
        let current: id = msg_send![panel, dataSource];
        if current == source() {
            let _: () = msg_send![panel, setDataSource: nil];
        }
    }
}
