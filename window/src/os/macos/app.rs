use crate::connection::ConnectionOps;
use crate::macos::menu::RepresentedItem;
use crate::macos::{nsstring, nsstring_to_str};
use crate::menu::{Menu, MenuItem};
use crate::{ApplicationEvent, Connection};
use cocoa::appkit::NSApplicationTerminateReply;
use cocoa::base::id;
use cocoa::foundation::NSInteger;
use config::keyassignment::KeyAssignment;
use config::WindowCloseConfirmation;
use objc::declare::ClassDecl;
use objc::rc::StrongPtr;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::*;

const CLS_NAME: &str = "WezTermAppDelegate";

extern "C" fn application_should_terminate(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> u64 {
    log::debug!("application termination requested");
    unsafe {
        match config::configuration().window_close_confirmation {
            WindowCloseConfirmation::NeverPrompt => terminate_now(),
            WindowCloseConfirmation::AlwaysPrompt => {
                let alert: id = msg_send![class!(NSAlert), alloc];
                let alert: id = msg_send![alert, init];
                let message_text = nsstring("Terminate WezTerm?");
                let info_text = nsstring("Detach and close all panes and terminate wezterm?");
                let cancel = nsstring("Cancel");
                let ok = nsstring("Ok");

                let () = msg_send![alert, setMessageText: message_text];
                let () = msg_send![alert, setInformativeText: info_text];
                let () = msg_send![alert, addButtonWithTitle: cancel];
                let () = msg_send![alert, addButtonWithTitle: ok];
                #[allow(non_upper_case_globals)]
                const NSModalResponseCancel: NSInteger = 1000;
                #[allow(non_upper_case_globals, dead_code)]
                const NSModalResponseOK: NSInteger = 1001;
                let result: NSInteger = msg_send![alert, runModal];
                log::info!("alert result is {result}");

                if result == NSModalResponseCancel {
                    NSApplicationTerminateReply::NSTerminateCancel as u64
                } else {
                    terminate_now()
                }
            }
        }
    }
}

fn terminate_now() -> u64 {
    if let Some(conn) = Connection::get() {
        conn.terminate_message_loop();
    }
    NSApplicationTerminateReply::NSTerminateNow as u64
}

extern "C" fn application_will_finish_launching(
    _self: &mut Object,
    _sel: Sel,
    _notif: *mut Object,
) {
    log::debug!("application_will_finish_launching");
}

thread_local! {
    /// Files and folders macOS asked us to open before launching
    /// finished, for example a folder dropped on the Dock icon while
    /// WezTerm wasn't running. They are opened once launching is done.
    static PENDING_OPEN_FILES: std::cell::RefCell<Vec<String>> =
        std::cell::RefCell::new(vec![]);
}

extern "C" fn application_did_finish_launching(this: &mut Object, _sel: Sel, _notif: *mut Object) {
    log::debug!("application_did_finish_launching");
    unsafe {
        (*this).set_ivar("launched", YES);
    }
    // Handle the Services menu items declared in Info.plist (NSServices).
    unsafe {
        let _: () = msg_send![cocoa::appkit::NSApp(), setServicesProvider: this as *mut Object];
    }
    let pending = PENDING_OPEN_FILES.with(|files| std::mem::take(&mut *files.borrow_mut()));
    if let Some(conn) = Connection::get() {
        for file_name in pending {
            log::debug!("opening {file_name}, which arrived before launch finished");
            conn.dispatch_app_event(ApplicationEvent::OpenCommandScript(file_name));
        }
    }
}

extern "C" fn application_open_untitled_file(
    this: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> BOOL {
    let launched: BOOL = unsafe { *this.get_ivar("launched") };
    log::debug!("application_open_untitled_file launched={launched}");
    if let Some(conn) = Connection::get() {
        if launched == YES {
            conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(
                KeyAssignment::SpawnWindow,
            ));
        }
        return YES;
    }
    NO
}

extern "C" fn wezterm_perform_key_assignment(
    _self: &mut Object,
    _sel: Sel,
    menu_item: *mut Object,
) {
    let menu_item = crate::os::macos::menu::MenuItem::with_menu_item(menu_item);
    // Safe because weztermPerformKeyAssignment: is only used with KeyAssignment
    let action = menu_item.get_represented_item();
    log::debug!("wezterm_perform_key_assignment {action:?}",);
    match action {
        Some(RepresentedItem::KeyAssignment(action)) => {
            if let Some(conn) = Connection::get() {
                conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(action));
            }
        }
        None => {}
    }
}

extern "C" fn application_open_file(
    this: &mut Object,
    _sel: Sel,
    _app: *mut Object,
    file_name: *mut Object,
) {
    let launched: BOOL = unsafe { *this.get_ivar("launched") };
    let file_name = unsafe { nsstring_to_str(file_name) }.to_string();
    log::debug!("application_open_file {file_name} launched={launched}");
    if launched == YES {
        if let Some(conn) = Connection::get() {
            conn.dispatch_app_event(ApplicationEvent::OpenCommandScript(file_name));
        }
    } else {
        PENDING_OPEN_FILES.with(|files| files.borrow_mut().push(file_name));
    }
}

extern "C" fn application_dock_menu(
    _self: &mut Object,
    _sel: Sel,
    _app: *mut Object,
) -> *mut Object {
    let dock_menu = Menu::new_with_title("");
    let new_window_item =
        MenuItem::new_with("New Window", Some(sel!(weztermPerformKeyAssignment:)), "");
    new_window_item
        .set_represented_item(RepresentedItem::KeyAssignment(KeyAssignment::SpawnWindow));
    dock_menu.add_item(&new_window_item);
    dock_menu.autorelease()
}

extern "C" fn application_did_change_active(_this: &mut Object, _sel: Sel, _notif: *mut Object) {
    super::secure_input::sync();
}

/// The folders in a Services pasteboard. A file stands for the folder
/// that holds it.
fn service_folders(pasteboard: *mut Object) -> Vec<String> {
    let mut folders = vec![];
    unsafe {
        let classes: id = msg_send![class!(NSArray), arrayWithObject: class!(NSURL)];
        let key = nsstring("NSPasteboardURLReadingFileURLsOnlyKey");
        let yes: id = msg_send![class!(NSNumber), numberWithBool: YES];
        let options: id = msg_send![class!(NSDictionary), dictionaryWithObject: yes forKey: *key];
        let urls: id = msg_send![pasteboard, readObjectsForClasses: classes options: options];
        if urls.is_null() {
            return folders;
        }
        let count: usize = msg_send![urls, count];
        for idx in 0..count {
            let url: id = msg_send![urls, objectAtIndex: idx];
            let path: id = msg_send![url, path];
            if path.is_null() {
                continue;
            }
            let path = std::path::PathBuf::from(nsstring_to_str(path));
            let folder = if path.is_dir() {
                Some(path)
            } else {
                path.parent().map(|p| p.to_path_buf())
            };
            if let Some(folder) = folder.and_then(|f| f.to_str().map(str::to_string)) {
                folders.push(folder);
            }
        }
    }
    folders
}

/// Services menu: "New WezTerm Window Here".
extern "C" fn service_open_window(
    _this: &mut Object,
    _sel: Sel,
    pasteboard: *mut Object,
    _user_data: *mut Object,
    _error: *mut std::ffi::c_void,
) {
    if let Some(conn) = Connection::get() {
        for folder in service_folders(pasteboard) {
            log::debug!("service: new window in {folder}");
            conn.dispatch_app_event(ApplicationEvent::OpenCommandScript(folder));
        }
    }
}

/// Services menu: "New WezTerm Tab Here".
extern "C" fn service_open_tab(
    _this: &mut Object,
    _sel: Sel,
    pasteboard: *mut Object,
    _user_data: *mut Object,
    _error: *mut std::ffi::c_void,
) {
    if let Some(conn) = Connection::get() {
        for folder in service_folders(pasteboard) {
            log::debug!("service: new tab in {folder}");
            conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(
                KeyAssignment::SpawnCommandInNewTab(config::keyassignment::SpawnCommand {
                    cwd: Some(folder.into()),
                    ..Default::default()
                }),
            ));
        }
    }
}

fn get_class() -> &'static Class {
    Class::get(CLS_NAME).unwrap_or_else(|| {
        let mut cls = ClassDecl::new(CLS_NAME, class!(NSObject))
            .expect("Unable to register application delegate class");

        cls.add_ivar::<BOOL>("launched");

        unsafe {
            cls.add_method(
                sel!(applicationShouldTerminate:),
                application_should_terminate as extern "C" fn(&mut Object, Sel, *mut Object) -> u64,
            );
            cls.add_method(
                sel!(applicationWillFinishLaunching:),
                application_will_finish_launching as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(applicationDidFinishLaunching:),
                application_did_finish_launching as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(application:openFile:),
                application_open_file as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object),
            );
            cls.add_method(
                sel!(applicationDidBecomeActive:),
                application_did_change_active as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(applicationDidResignActive:),
                application_did_change_active as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(applicationDockMenu:),
                application_dock_menu
                    as extern "C" fn(&mut Object, Sel, *mut Object) -> *mut Object,
            );
            cls.add_method(
                sel!(weztermPerformKeyAssignment:),
                wezterm_perform_key_assignment as extern "C" fn(&mut Object, Sel, *mut Object),
            );
            cls.add_method(
                sel!(weztermServiceOpenWindow:userData:error:),
                service_open_window
                    as extern "C" fn(
                        &mut Object,
                        Sel,
                        *mut Object,
                        *mut Object,
                        *mut std::ffi::c_void,
                    ),
            );
            cls.add_method(
                sel!(weztermServiceOpenTab:userData:error:),
                service_open_tab
                    as extern "C" fn(
                        &mut Object,
                        Sel,
                        *mut Object,
                        *mut Object,
                        *mut std::ffi::c_void,
                    ),
            );
            cls.add_method(
                sel!(applicationOpenUntitledFile:),
                application_open_untitled_file
                    as extern "C" fn(&mut Object, Sel, *mut Object) -> BOOL,
            );
        }

        cls.register()
    })
}

pub fn create_app_delegate() -> StrongPtr {
    let cls = get_class();
    unsafe {
        let delegate: *mut Object = msg_send![cls, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        (*delegate).set_ivar("launched", NO);
        StrongPtr::new(delegate)
    }
}
