//! A system-wide hotkey that shows and hides WezTerm, like iTerm2's
//! hotkey. It uses Carbon's RegisterEventHotKey, which works without
//! the Accessibility permission that a global event monitor needs.

use super::keycodes::phys_to_vkey;
use crate::connection::ConnectionOps;
use crate::{ApplicationEvent, Connection};
use cocoa::appkit::NSApp;
use cocoa::base::{id, nil, YES};
use config::keyassignment::KeyAssignment;
use objc::*;
use std::cell::Cell;
use std::ffi::c_void;
use wezterm_input_types::{KeyCode, Modifiers};

type OSStatus = i32;
type EventTargetRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventRef = *mut c_void;
type EventHotKeyRef = *mut c_void;
type EventHandlerUPP = extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerUPP,
        num_types: u32,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut EventHandlerRef,
    ) -> OSStatus;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        out_ref: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn UnregisterEventHotKey(hot_key: EventHotKeyRef) -> OSStatus;
}

const K_EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;
const SIGNATURE: u32 = u32::from_be_bytes(*b"WezT");

// Carbon modifier bits.
const CMD_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const OPTION_KEY: u32 = 1 << 11;
const CONTROL_KEY: u32 = 1 << 12;

thread_local! {
    static HANDLER_INSTALLED: Cell<bool> = Cell::new(false);
    static REGISTERED: Cell<EventHotKeyRef> = Cell::new(std::ptr::null_mut());
}

extern "C" fn hot_key_pressed(
    _next: EventHandlerCallRef,
    _event: EventRef,
    _user_data: *mut c_void,
) -> OSStatus {
    toggle_app_visibility();
    0
}

fn carbon_modifiers(mods: Modifiers) -> u32 {
    let mut result = 0;
    if mods.contains(Modifiers::SUPER) {
        result |= CMD_KEY;
    }
    if mods.contains(Modifiers::SHIFT) {
        result |= SHIFT_KEY;
    }
    if mods.contains(Modifiers::ALT) {
        result |= OPTION_KEY;
    }
    if mods.contains(Modifiers::CTRL) {
        result |= CONTROL_KEY;
    }
    result
}

/// Register `hotkey` as the system-wide hotkey, replacing any earlier
/// one, or remove it with `None`. Main thread only.
pub fn set_global_hotkey(hotkey: Option<(KeyCode, Modifiers)>) -> anyhow::Result<()> {
    REGISTERED.with(|registered| {
        let old = registered.replace(std::ptr::null_mut());
        if !old.is_null() {
            unsafe { UnregisterEventHotKey(old) };
        }
    });
    let Some((key, mods)) = hotkey else {
        return Ok(());
    };
    let phys = key
        .to_phys()
        .ok_or_else(|| anyhow::anyhow!("{key:?} has no physical key"))?;
    let vkey =
        phys_to_vkey(phys).ok_or_else(|| anyhow::anyhow!("no macOS key code for {phys:?}"))?;

    unsafe {
        if !HANDLER_INSTALLED.with(|i| i.get()) {
            let spec = EventTypeSpec {
                event_class: K_EVENT_CLASS_KEYBOARD,
                event_kind: K_EVENT_HOT_KEY_PRESSED,
            };
            let status = InstallEventHandler(
                GetApplicationEventTarget(),
                hot_key_pressed,
                1,
                &spec,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            anyhow::ensure!(status == 0, "InstallEventHandler failed: {status}");
            HANDLER_INSTALLED.with(|i| i.set(true));
        }

        let mut hot_key: EventHotKeyRef = std::ptr::null_mut();
        let status = RegisterEventHotKey(
            vkey as u32,
            carbon_modifiers(mods),
            EventHotKeyID {
                signature: SIGNATURE,
                id: 1,
            },
            GetApplicationEventTarget(),
            0,
            &mut hot_key,
        );
        anyhow::ensure!(
            status == 0,
            "RegisterEventHotKey failed: {status}; another app may already use this hotkey"
        );
        REGISTERED.with(|registered| registered.set(hot_key));
    }
    log::debug!("registered global hotkey {key:?} {mods:?} (vkey {vkey})");
    Ok(())
}

/// Hide WezTerm when it is the active app; otherwise bring it to the
/// front, opening a window if there is none.
pub fn toggle_app_visibility() {
    unsafe {
        let app = NSApp();
        let active: bool = msg_send![app, isActive];
        let hidden: bool = msg_send![app, isHidden];
        if active && !hidden {
            let () = msg_send![app, hide: nil];
            return;
        }
        let () = msg_send![app, unhide: nil];
        let () = msg_send![app, activateIgnoringOtherApps: YES];

        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        let mut visible = false;
        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex: i];
            let is_visible: bool = msg_send![window, isVisible];
            let miniaturized: bool = msg_send![window, isMiniaturized];
            if is_visible || miniaturized {
                visible = true;
                break;
            }
        }
        if !visible {
            if let Some(conn) = Connection::get() {
                conn.dispatch_app_event(ApplicationEvent::PerformKeyAssignment(
                    KeyAssignment::SpawnWindow,
                ));
            }
        }
    }
}
