//! Keyboard events via CGEvent.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

type CFTypeRef = *const c_void;

const CG_HID_EVENT_TAP: u32 = 0;

// Modifier flags
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: CFTypeRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CFTypeRef;
    fn CGEventSetFlags(event: CFTypeRef, flags: u64);
    fn CGEventPost(tap: u32, event: CFTypeRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

/// Send a key combo like "cmd+c", "enter", "cmd+shift+s".
pub fn send(combo: &str) -> Result<(), String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("empty key combo".into());
    }

    let key_name = parts.last().unwrap();
    let modifier_names = &parts[..parts.len() - 1];

    let keycode = resolve_keycode(key_name)?;
    let flags = resolve_flags(modifier_names)?;

    unsafe {
        let down = CGEventCreateKeyboardEvent(std::ptr::null(), keycode, true);
        if down.is_null() {
            return Err("failed to create key-down event".into());
        }

        let up = CGEventCreateKeyboardEvent(std::ptr::null(), keycode, false);
        if up.is_null() {
            CFRelease(down);
            return Err("failed to create key-up event".into());
        }

        if flags != 0 {
            CGEventSetFlags(down, flags);
            CGEventSetFlags(up, flags);
        }

        CGEventPost(CG_HID_EVENT_TAP, down);
        CGEventPost(CG_HID_EVENT_TAP, up);

        CFRelease(down);
        CFRelease(up);
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
