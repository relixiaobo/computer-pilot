//! Keyboard events via CGEvent.
//!
//! ## Targeting
//!
//! Both `send` and `type_text` accept `target_pid: Option<i32>`:
//! - `Some(pid)` → events delivered only to that process via `CGEventPostToPid`
//!   using a combined-session event source. Doesn't steal focus, doesn't go
//!   through the global keyboard hook, doesn't interact with IME state. Used
//!   when the agent knows the target app.
//! - `None` → events posted to the global HID tap. Behaves like a real
//!   keypress; goes to whatever is frontmost. Useful when the user wants
//!   the agent to operate on the foreground app explicitly.
//!
//! `type_text` uses `CGEventKeyboardSetUnicodeString` with virtual_key=0,
//! one UTF-16 code unit per event. This bypasses the keyboard layout and
//! IME entirely — Chinese, emoji, and other non-ASCII text Just Works.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

type CFTypeRef = *const c_void;

const CG_HID_EVENT_TAP: u32 = 0;
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: i32 = 0;

// Modifier flags
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: CFTypeRef, virtual_key: u16, key_down: bool)
    -> CFTypeRef;
    fn CGEventKeyboardSetUnicodeString(event: CFTypeRef, length: usize, string: *const u16);
    fn CGEventSetFlags(event: CFTypeRef, flags: u64);
    fn CGEventPost(tap: u32, event: CFTypeRef);
    fn CGEventPostToPid(pid: i32, event: CFTypeRef);
    fn CGEventSourceCreate(state_id: i32) -> CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

/// RAII wrapper for a CGEventSource. Held for the duration of a key sequence so
/// every event in the sequence shares the same session-level source. Null when
/// targeting the global HID tap.
struct EventSource(CFTypeRef);

impl EventSource {
    fn for_target(target_pid: Option<i32>) -> Self {
        let raw = if target_pid.is_some() {
            unsafe { CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE) }
        } else {
            std::ptr::null()
        };
        EventSource(raw)
    }

    fn raw(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for EventSource {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

unsafe fn post(event: CFTypeRef, target_pid: Option<i32>) {
    match target_pid {
        Some(pid) => CGEventPostToPid(pid, event),
        None => CGEventPost(CG_HID_EVENT_TAP, event),
    }
}

/// Send a key combo like "cmd+c", "enter", "cmd+shift+s".
/// See module docs for `target_pid` semantics.
pub fn send(combo: &str, target_pid: Option<i32>) -> Result<(), String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("empty key combo".into());
    }

    let key_name = parts.last().unwrap();
    let modifier_names = &parts[..parts.len() - 1];

    let keycode = resolve_keycode(key_name)?;
    let flags = resolve_flags(modifier_names)?;

    let source = EventSource::for_target(target_pid);
    unsafe {
        let down = CGEventCreateKeyboardEvent(source.raw(), keycode, true);
        if down.is_null() {
            return Err("failed to create key-down event".into());
        }

        let up = CGEventCreateKeyboardEvent(source.raw(), keycode, false);
        if up.is_null() {
            CFRelease(down);
            return Err("failed to create key-up event".into());
        }

        if flags != 0 {
            CGEventSetFlags(down, flags);
            CGEventSetFlags(up, flags);
        }

        post(down, target_pid);
        post(up, target_pid);

        CFRelease(down);
        CFRelease(up);
    }

    Ok(())
}

/// Type Unicode text into the target. One UTF-16 code unit per key event,
/// using virtual_key=0 + `CGEventKeyboardSetUnicodeString` so keyboard layout
/// and IME are bypassed entirely. Works for any language, any script.
///
/// `target_pid: Some(pid)` → delivered to that process (no focus theft, no
/// clipboard pollution, no app activation).
/// `target_pid: None` → goes to whatever app is frontmost (global HID tap).
pub fn type_text(text: &str, target_pid: Option<i32>) -> Result<(), String> {
    let source = EventSource::for_target(target_pid);
    let utf16: Vec<u16> = text.encode_utf16().collect();

    for ch in utf16 {
        let mut buf = ch;
        unsafe {
            let down = CGEventCreateKeyboardEvent(source.raw(), 0, true);
            if down.is_null() {
                return Err("failed to create key-down event for type".into());
            }
            let up = CGEventCreateKeyboardEvent(source.raw(), 0, false);
            if up.is_null() {
                CFRelease(down);
                return Err("failed to create key-up event for type".into());
            }

            CGEventKeyboardSetUnicodeString(down, 1, &mut buf as *mut u16);
            CGEventKeyboardSetUnicodeString(up, 1, &mut buf as *mut u16);

            post(down, target_pid);
            post(up, target_pid);

            CFRelease(down);
            CFRelease(up);
        }
        // Inter-event gap: HID drops events that arrive faster than the tap
        // can drain. 3ms is empirically enough (kagete uses the same value).
        std::thread::sleep(std::time::Duration::from_micros(3000));
    }

    Ok(())
}

fn resolve_flags(names: &[&str]) -> Result<u64, String> {
    let mut flags = 0u64;
    for name in names {
        flags |= match name.to_lowercase().as_str() {
            "cmd" | "command" => FLAG_COMMAND,
            "shift" => FLAG_SHIFT,
            "ctrl" | "control" => FLAG_CONTROL,
            "alt" | "option" | "opt" => FLAG_OPTION,
            other => return Err(format!("unknown modifier: {other}")),
        };
    }
    Ok(flags)
}

pub fn resolve_keycode(name: &str) -> Result<u16, String> {
    let code = match name.to_lowercase().as_str() {
        // Letters
        "a" => 0,
        "b" => 11,
        "c" => 8,
        "d" => 2,
        "e" => 14,
        "f" => 3,
        "g" => 5,
        "h" => 4,
        "i" => 34,
        "j" => 38,
        "k" => 40,
        "l" => 37,
        "m" => 46,
        "n" => 45,
        "o" => 31,
        "p" => 35,
        "q" => 12,
        "r" => 15,
        "s" => 1,
        "t" => 17,
        "u" => 32,
        "v" => 9,
        "w" => 13,
        "x" => 7,
        "y" => 16,
        "z" => 6,

        // Numbers
        "0" => 29,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,

        // Special keys
        "return" | "enter" => 36,
        "tab" => 48,
        "space" => 49,
        "delete" | "backspace" => 51,
        "escape" | "esc" => 53,
        "forwarddelete" => 117,

        // Arrow keys
        "up" => 126,
        "down" => 125,
        "left" => 123,
        "right" => 124,

        // Punctuation
        "-" | "minus" => 27,
        "=" | "equal" | "plus" => 24,
        "[" | "leftbracket" => 33,
        "]" | "rightbracket" => 30,
        ";" | "semicolon" => 41,
        "'" | "quote" => 39,
        "," | "comma" => 43,
        "." | "period" => 47,
        "/" | "slash" => 44,
        "\\" | "backslash" => 42,
        "`" | "grave" => 50,

        // Function keys
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,

        // Page navigation
        "pageup" => 116,
        "pagedown" => 121,
        "home" => 115,
        "end" => 119,

        other => return Err(format!("unknown key: {other}")),
    };

    Ok(code)
}
