//! Secure Keyboard Entry, as in Terminal.app and iTerm2.
//!
//! While it is on and WezTerm is the active app, macOS stops other
//! processes from reading keystrokes with event taps. It is turned off
//! whenever WezTerm stops being the active app, because secure input
//! applies to the whole login session while it is enabled.

use cocoa::appkit::NSApp;
use objc::runtime::{BOOL, YES};
use objc::*;
use std::sync::atomic::{AtomicBool, Ordering};

extern "C" {
    // Carbon (HIToolbox); window/build.rs links the framework.
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

/// What the user asked for.
static WANTED: AtomicBool = AtomicBool::new(false);
/// Whether we currently hold an EnableSecureEventInput call.
/// Calls must be balanced: macOS keeps a count.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn Secure Keyboard Entry on or off. Main thread only.
pub fn set_secure_keyboard_entry(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
    sync();
}

/// Whether Secure Keyboard Entry is turned on. It only takes effect
/// while WezTerm is the active app.
pub fn secure_keyboard_entry() -> bool {
    WANTED.load(Ordering::Relaxed)
}

/// Enable or disable secure input to match the setting and whether the
/// app is active. Called when either changes.
pub(crate) fn sync() {
    let active: BOOL = unsafe { msg_send![NSApp(), isActive] };
    let want = WANTED.load(Ordering::Relaxed) && active == YES;
    if want != ENABLED.load(Ordering::Relaxed) {
        let status = unsafe {
            if want {
                EnableSecureEventInput()
            } else {
                DisableSecureEventInput()
            }
        };
        if status != 0 {
            log::warn!("secure event input: {want} failed with status {status}");
            return;
        }
        ENABLED.store(want, Ordering::Relaxed);
        log::debug!("secure event input enabled={want}");
    }
}
