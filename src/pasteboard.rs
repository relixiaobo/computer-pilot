//! Full-fidelity NSPasteboard save/restore for the clipboard-paste typing path.
//!
//! pbcopy/pbpaste only round-trip plain text — an image, file, or rich-text
//! clipboard would be silently destroyed by the paste route. This module
//! snapshots every pasteboard item with every type it carries and writes it
//! all back after the paste. `change_count()` lets the caller detect a user
//! (or third-party) write during the paste window so we never overwrite
//! content the user just copied.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, c_char, c_void};

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

const NS_UTF8_STRING_ENCODING: usize = 4;

type SendId = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type SendIdArg = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type SendIndex = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void;
type SendUsize = unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize;
type SendI64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64;
type SendVoidPtr = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_void;
type SendCStr = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char;
type SendBool2 = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> bool;
type SendBool1 = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;
type SendBytes =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize) -> *mut c_void;
type SendStrBytes =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, usize, usize) -> *mut c_void;
type SendArray =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void, usize) -> *mut c_void;

unsafe fn send_id() -> SendId {
    std::mem::transmute(objc_msgSend as *const ())
}

unsafe fn sel(name: &CStr) -> *mut c_void {
    sel_registerName(name.as_ptr())
}

/// Everything the general pasteboard held: one entry per pasteboard item,
/// each a list of `(type UTI, raw data)` pairs.
pub struct SavedClipboard {
    items: Vec<Vec<(String, Vec<u8>)>>,
    /// Types whose data could not be read (unresolvable promises etc.).
    pub skipped_types: usize,
}

impl SavedClipboard {
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

unsafe fn general_pasteboard() -> Result<*mut c_void, String> {
    let class = objc_getClass(c"NSPasteboard".as_ptr());
    if class.is_null() {
        return Err("NSPasteboard class unavailable".into());
    }
    let pb = send_id()(class, sel(c"generalPasteboard"));
    if pb.is_null() {
        return Err("generalPasteboard returned nil".into());
    }
    Ok(pb)
}

/// Alloc/init an NSString from arbitrary UTF-8 (handles interior NULs, unlike
/// stringWithUTF8String:). Caller must `release`.
unsafe fn nsstring_alloc(text: &str) -> *mut c_void {
    let class = objc_getClass(c"NSString".as_ptr());
    if class.is_null() {
        return std::ptr::null_mut();
    }
    let alloc = send_id()(class, sel(c"alloc"));
    if alloc.is_null() {
        return std::ptr::null_mut();
    }
    let init: SendStrBytes = std::mem::transmute(objc_msgSend as *const ());
    init(
        alloc,
        sel(c"initWithBytes:length:encoding:"),
        text.as_ptr() as *const c_void,
        text.len(),
        NS_UTF8_STRING_ENCODING,
    )
}

unsafe fn release(obj: *mut c_void) {
    if !obj.is_null() {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        send(obj, sel(c"release"));
    }
}

/// The pasteboard's current change count. Increments on every write from any
/// process — the caller uses this to detect a user write mid-paste.
pub fn change_count() -> Result<i64, String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = general_pasteboard().map(|pb| {
            let send: SendI64 = std::mem::transmute(objc_msgSend as *const ());
            send(pb, sel(c"changeCount"))
        });
        objc_autoreleasePoolPop(pool);
        result
    }
}

/// Snapshot every item and every type on the general pasteboard.
pub fn save() -> Result<SavedClipboard, String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = save_inner();
        objc_autoreleasePoolPop(pool);
        result
    }
}

unsafe fn save_inner() -> Result<SavedClipboard, String> {
    let pb = general_pasteboard()?;
    let send_index: SendIndex = std::mem::transmute(objc_msgSend as *const ());
    let send_usize: SendUsize = std::mem::transmute(objc_msgSend as *const ());
    let send_id_arg: SendIdArg = std::mem::transmute(objc_msgSend as *const ());
    let send_ptr: SendVoidPtr = std::mem::transmute(objc_msgSend as *const ());
    let send_cstr: SendCStr = std::mem::transmute(objc_msgSend as *const ());

    let mut saved = SavedClipboard {
        items: Vec::new(),
        skipped_types: 0,
    };

    // Nil pasteboardItems means the pasteboard is empty (or unreadable) —
    // treat as empty; restore will just clear back to empty.
    let items = send_id()(pb, sel(c"pasteboardItems"));
    let item_count = if items.is_null() {
        0
    } else {
        send_usize(items, sel(c"count"))
    };

    for i in 0..item_count {
        let item = send_index(items, sel(c"objectAtIndex:"), i);
        if item.is_null() {
            continue;
        }
        let types = send_id()(item, sel(c"types"));
        let type_count = if types.is_null() {
            0
        } else {
            send_usize(types, sel(c"count"))
        };
        let mut entry: Vec<(String, Vec<u8>)> = Vec::with_capacity(type_count);
        for t in 0..type_count {
            let type_ns = send_index(types, sel(c"objectAtIndex:"), t);
            if type_ns.is_null() {
                saved.skipped_types += 1;
                continue;
            }
            let utf8 = send_cstr(type_ns, sel(c"UTF8String"));
            if utf8.is_null() {
                saved.skipped_types += 1;
                continue;
            }
            let uti = CStr::from_ptr(utf8).to_string_lossy().into_owned();
            // dataForType: resolves lazy/promised data; nil means the promise
            // could not be fulfilled — count it so the caller can surface it.
            let data = send_id_arg(item, sel(c"dataForType:"), type_ns);
            if data.is_null() {
                saved.skipped_types += 1;
                continue;
            }
            let len = send_usize(data, sel(c"length"));
            let bytes = send_ptr(data, sel(c"bytes"));
            let mut buf = Vec::with_capacity(len);
            if len > 0 {
                if bytes.is_null() {
                    saved.skipped_types += 1;
                    continue;
                }
                buf.extend_from_slice(std::slice::from_raw_parts(bytes as *const u8, len));
            }
            entry.push((uti, buf));
        }
        if !entry.is_empty() {
            saved.items.push(entry);
        }
    }
    Ok(saved)
}

/// Clear the pasteboard and write `text` as plain UTF-8. Returns the
/// pasteboard change count after the write.
pub fn set_text(text: &str) -> Result<i64, String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = set_text_inner(text);
        objc_autoreleasePoolPop(pool);
        result
    }
}

unsafe fn set_text_inner(text: &str) -> Result<i64, String> {
    let pb = general_pasteboard()?;
    let send_i64: SendI64 = std::mem::transmute(objc_msgSend as *const ());
    let set_string: SendBool2 = std::mem::transmute(objc_msgSend as *const ());

    send_i64(pb, sel(c"clearContents"));
    let ns_text = nsstring_alloc(text);
    if ns_text.is_null() {
        return Err("failed to create NSString for clipboard text".into());
    }
    let ns_type = nsstring_alloc("public.utf8-plain-text");
    if ns_type.is_null() {
        release(ns_text);
        return Err("failed to create NSString for pasteboard type".into());
    }
    let ok = set_string(pb, sel(c"setString:forType:"), ns_text, ns_type);
    release(ns_text);
    release(ns_type);
    if !ok {
        return Err("NSPasteboard setString:forType: failed".into());
    }
    Ok(send_i64(pb, sel(c"changeCount")))
}

/// Write a saved snapshot back to the general pasteboard.
pub fn restore(saved: &SavedClipboard) -> Result<(), String> {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = restore_inner(saved);
        objc_autoreleasePoolPop(pool);
        result
    }
}

unsafe fn restore_inner(saved: &SavedClipboard) -> Result<(), String> {
    let pb = general_pasteboard()?;
    let send_i64: SendI64 = std::mem::transmute(objc_msgSend as *const ());
    send_i64(pb, sel(c"clearContents"));
    if saved.items.is_empty() {
        // Original clipboard was empty — cleared is fully restored.
        return Ok(());
    }

    let item_class = objc_getClass(c"NSPasteboardItem".as_ptr());
    let data_class = objc_getClass(c"NSData".as_ptr());
    let array_class = objc_getClass(c"NSArray".as_ptr());
    if item_class.is_null() || data_class.is_null() || array_class.is_null() {
        return Err("AppKit pasteboard classes unavailable".into());
    }
    let data_with_bytes: SendBytes = std::mem::transmute(objc_msgSend as *const ());
    let set_data: SendBool2 = std::mem::transmute(objc_msgSend as *const ());
    let array_with: SendArray = std::mem::transmute(objc_msgSend as *const ());
    let write_objects: SendBool1 = std::mem::transmute(objc_msgSend as *const ());

    let mut ns_items: Vec<*mut c_void> = Vec::with_capacity(saved.items.len());
    let mut build_err: Option<String> = None;
    'build: for entry in &saved.items {
        let alloc = send_id()(item_class, sel(c"alloc"));
        let item = send_id()(alloc, sel(c"init"));
        if item.is_null() {
            build_err = Some("failed to create NSPasteboardItem".into());
            break;
        }
        for (uti, bytes) in entry {
            let ptr = if bytes.is_empty() {
                std::ptr::null()
            } else {
                bytes.as_ptr() as *const c_void
            };
            // Autoreleased NSData — the pool around restore() reclaims it.
            let data = data_with_bytes(data_class, sel(c"dataWithBytes:length:"), ptr, bytes.len());
            let ns_type = nsstring_alloc(uti);
            if data.is_null() || ns_type.is_null() {
                release(ns_type);
                release(item);
                build_err = Some(format!("failed to rebuild pasteboard type {uti}"));
                break 'build;
            }
            let ok = set_data(item, sel(c"setData:forType:"), data, ns_type);
            release(ns_type);
            if !ok {
                release(item);
                build_err = Some(format!("setData:forType: failed for {uti}"));
                break 'build;
            }
        }
        ns_items.push(item);
    }

    let result = match build_err {
        Some(err) => Err(err),
        None => {
            let array = array_with(
                array_class,
                sel(c"arrayWithObjects:count:"),
                ns_items.as_ptr(),
                ns_items.len(),
            );
            if array.is_null() {
                Err("failed to build NSArray of pasteboard items".into())
            } else if write_objects(pb, sel(c"writeObjects:"), array) {
                Ok(())
            } else {
                Err("NSPasteboard writeObjects: failed".into())
            }
        }
    };
    for item in ns_items {
        release(item);
    }
    result
}
