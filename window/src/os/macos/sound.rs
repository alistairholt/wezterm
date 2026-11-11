//! Play a named system sound or a sound file, for a custom bell.

use super::nsstring;
use cocoa::base::{id, nil, YES};
use objc::rc::StrongPtr;
use objc::*;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Loaded sounds by name or path; None when loading failed, so a
    /// bad name is only reported once.
    static SOUNDS: RefCell<HashMap<String, Option<StrongPtr>>> = RefCell::new(HashMap::new());
}

fn load(name_or_path: &str) -> Option<StrongPtr> {
    unsafe {
        let sound: id = if name_or_path.contains('/') {
            let path = nsstring(name_or_path);
            let sound: id = msg_send![class!(NSSound), alloc];
            msg_send![sound, initWithContentsOfFile: *path byReference: YES]
        } else {
            let name = nsstring(name_or_path);
            let sound: id = msg_send![class!(NSSound), soundNamed: *name];
            if sound != nil {
                let _: id = msg_send![sound, retain];
            }
            sound
        };
        if sound == nil {
            None
        } else {
            Some(StrongPtr::new(sound))
        }
    }
}

/// Play `name_or_path`: a system sound name such as "Glass", or the
/// path of a sound file. Returns false if it couldn't be loaded.
/// Main thread only.
pub fn play_sound(name_or_path: &str) -> bool {
    SOUNDS.with(|sounds| {
        let mut sounds = sounds.borrow_mut();
        let sound = sounds.entry(name_or_path.to_string()).or_insert_with(|| {
            let sound = load(name_or_path);
            if sound.is_none() {
                log::error!("couldn't load the sound {name_or_path:?}");
            }
            sound
        });
        let Some(sound) = sound else {
            return false;
        };
        unsafe {
            // Playing a sound that is still playing does nothing, so
            // start it again from the beginning.
            let _: bool = msg_send![**sound, stop];
            let _: bool = msg_send![**sound, play];
            let playing: bool = msg_send![**sound, isPlaying];
            log::debug!("playing sound {name_or_path:?}: isPlaying={playing}");
        }
        true
    })
}
