//! Sdef (Scripting Definition) parser — pure Rust, no osascript.
//! Reads .sdef XML and converts legacy Cocoa dictionaries through macOS OSA.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::ffi::c_void;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
pub struct SdefResult {
    pub ok: bool,
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suites: Option<Vec<Suite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Suite {
    pub name: String,
    pub classes: Vec<Class>,
    pub commands: Vec<SdefCommand>,
}

#[derive(serde::Serialize)]
pub struct Class {
    pub name: String,
    pub properties: Vec<Property>,
    pub elements: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub responds_to: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct Property {
    pub name: String,
    pub access: String,
}

#[derive(serde::Serialize)]
pub struct SdefCommand {
    pub name: String,
    pub description: String,
    pub parameters: Vec<String>,
}

// Cocoa apps can ship .scriptSuite instead of .sdef. Ask the OS to convert
// that legacy format, then feed the same XML parser used for modern bundles.
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn OSACopyScriptingDefinitionFromURL(
        url: *const c_void,
        flags: i32,
        data: *mut *const c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: *const c_void,
        bytes: *const u8,
        length: isize,
        directory: u8,
    ) -> *const c_void;
    fn CFDataGetLength(data: *const c_void) -> isize;
    fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
    fn CFRelease(value: *const c_void);
}

fn definition_path(bundle_path: &str) -> Option<PathBuf> {
    let mut legacy = None;
    for entry in std::fs::read_dir(Path::new(bundle_path).join("Contents/Resources"))
        .ok()?
        .flatten()
    {
        let path = entry.path();
        match path.extension().and_then(|value| value.to_str()) {
            Some("sdef") => return Some(path),
            Some("scriptSuite") => legacy = Some(path),
            _ => {}
        }
    }
    legacy
}

fn read_definition(bundle_path: &str) -> Result<String, String> {
    let path = definition_path(bundle_path).ok_or("not scriptable (no scripting dictionary)")?;
    if path.extension().is_some_and(|ext| ext == "sdef") {
        return std::fs::read_to_string(path).map_err(|e| format!("failed to read sdef: {e}"));
    }
    // OSA may request dynamic terminology from the app. Only explicit sdef
    // discovery uses it; listing apps must remain a local metadata read.
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bundle_path.as_ptr(),
            bundle_path.len() as isize,
            1,
        );
        if url.is_null() {
            return Err("failed to create scripting dictionary URL".into());
        }
        let mut data = std::ptr::null();
        let status = OSACopyScriptingDefinitionFromURL(url, 0, &mut data);
        CFRelease(url);
        if status != 0 || data.is_null() {
            if !data.is_null() {
                CFRelease(data);
            }
            return Err(format!(
                "failed to load scripting dictionary (OSA error {status})"
            ));
        }
        let length = CFDataGetLength(data) as usize;
        let xml = if length == 0 {
            Err("empty scripting dictionary".into())
        } else {
            let bytes = std::slice::from_raw_parts(CFDataGetBytePtr(data), length);
            String::from_utf8(bytes.to_vec())
                .map_err(|e| format!("invalid scripting dictionary UTF-8: {e}"))
        };
        CFRelease(data);
        xml
    }
}

/// Detect scripting support without launching or sending events to an app.
/// A legacy dictionary has no XML class count until explicitly converted.
pub fn dictionary_info(bundle_path: &str) -> (bool, Option<usize>) {
    let Some(path) = definition_path(bundle_path) else {
        return (false, None);
    };
    if path.extension().is_some_and(|ext| ext == "scriptSuite") {
        return (true, None);
    }
    let Ok(xml) = std::fs::read_to_string(path) else {
        return (false, None);
    };
    let mut reader = Reader::from_str(&xml);
    let mut count = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                if matches!(e.name().as_ref(), b"class" | b"class-extension") {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return (false, None),
            _ => {}
        }
    }
    (true, Some(count))
}

/// Parse the full sdef and return structured result.
pub fn parse(app: &str, bundle_path: &str) -> SdefResult {
    let xml = match read_definition(bundle_path) {
        Ok(xml) => xml,
        Err(error) => {
            return SdefResult {
                ok: false,
                app: app.to_string(),
                suites: None,
                error: Some(format!("{app}: {error}")),
            };
        }
    };

    let mut reader = Reader::from_str(&xml);
    let mut suites = Vec::new();

    // State machine
    enum State {
        Root,
        Suite,
        Class,
        ClassExt,
        Command,
    }
    let mut state = State::Root;
    let mut current_suite = Option::<Suite>::None;
    let mut current_class = Option::<Class>::None;
    let mut current_cmd = Option::<SdefCommand>::None;

    fn attr_val(e: &quick_xml::events::BytesStart, key: &[u8]) -> String {
        e.attributes()
            .flatten()
            .find(|a| a.key.as_ref() == key)
            .map(|a| String::from_utf8_lossy(&a.value).to_string())
            .unwrap_or_default()
    }

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"suite" => {
                        if let Some(s) = current_suite.take()
                            && (!s.classes.is_empty() || !s.commands.is_empty())
                        {
                            suites.push(s);
                        }
                        current_suite = Some(Suite {
                            name: attr_val(e, b"name"),
                            classes: Vec::new(),
                            commands: Vec::new(),
                        });
                        state = State::Suite;
                    }
                    b"class" if matches!(state, State::Suite) => {
                        current_class = Some(Class {
                            name: attr_val(e, b"name"),
                            properties: Vec::new(),
                            elements: Vec::new(),
                            responds_to: Vec::new(),
                        });
                        state = State::Class;
                    }
                    b"class-extension" if matches!(state, State::Suite) => {
                        current_class = Some(Class {
                            name: attr_val(e, b"extends"),
                            properties: Vec::new(),
                            elements: Vec::new(),
                            responds_to: Vec::new(),
                        });
                        state = State::ClassExt;
                    }
                    b"command" if matches!(state, State::Suite) => {
                        current_cmd = Some(SdefCommand {
                            name: attr_val(e, b"name"),
                            description: attr_val(e, b"description"),
                            parameters: Vec::new(),
                        });
                        state = State::Command;
                    }
                    b"property" if matches!(state, State::Class | State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            let access = attr_val(e, b"access");
                            cls.properties.push(Property {
                                name: attr_val(e, b"name"),
                                access: if access.is_empty() {
                                    "rw".to_string()
                                } else {
                                    access
                                },
                            });
                        }
                    }
                    b"element" if matches!(state, State::Class | State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            cls.elements.push(attr_val(e, b"type"));
                        }
                    }
                    b"responds-to" if matches!(state, State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            cls.responds_to.push(attr_val(e, b"command"));
                        }
                    }
                    b"parameter" if matches!(state, State::Command) => {
                        if let Some(ref mut cmd) = current_cmd {
                            cmd.parameters.push(attr_val(e, b"name"));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"property" if matches!(state, State::Class | State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            let access = attr_val(e, b"access");
                            cls.properties.push(Property {
                                name: attr_val(e, b"name"),
                                access: if access.is_empty() {
                                    "rw".to_string()
                                } else {
                                    access
                                },
                            });
                        }
                    }
                    b"element" if matches!(state, State::Class | State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            cls.elements.push(attr_val(e, b"type"));
                        }
                    }
                    b"responds-to" if matches!(state, State::ClassExt) => {
                        if let Some(ref mut cls) = current_class {
                            cls.responds_to.push(attr_val(e, b"command"));
                        }
                    }
                    b"parameter" if matches!(state, State::Command) => {
                        if let Some(ref mut cmd) = current_cmd {
                            cmd.parameters.push(attr_val(e, b"name"));
                        }
                    }
                    // Self-closing class/class-extension/command in suite context
                    b"class" if matches!(state, State::Suite) => {
                        if let Some(ref mut suite) = current_suite {
                            suite.classes.push(Class {
                                name: attr_val(e, b"name"),
                                properties: Vec::new(),
                                elements: Vec::new(),
                                responds_to: Vec::new(),
                            });
                        }
                    }
                    b"class-extension" if matches!(state, State::Suite) => {
                        if let Some(ref mut suite) = current_suite {
                            suite.classes.push(Class {
                                name: attr_val(e, b"extends"),
                                properties: Vec::new(),
                                elements: Vec::new(),
                                responds_to: Vec::new(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"class" if matches!(state, State::Class) => {
                    if let (Some(cls), Some(suite)) = (current_class.take(), &mut current_suite) {
                        suite.classes.push(cls);
                    }
                    state = State::Suite;
                }
                b"class-extension" if matches!(state, State::ClassExt) => {
                    if let (Some(cls), Some(suite)) = (current_class.take(), &mut current_suite) {
                        suite.classes.push(cls);
                    }
                    state = State::Suite;
                }
                b"command" if matches!(state, State::Command) => {
                    if let (Some(cmd), Some(suite)) = (current_cmd.take(), &mut current_suite) {
                        suite.commands.push(cmd);
                    }
                    state = State::Suite;
                }
                b"suite" => {
                    if let Some(s) = current_suite.take()
                        && (!s.classes.is_empty() || !s.commands.is_empty())
                    {
                        suites.push(s);
                    }
                    state = State::Root;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return SdefResult {
                    ok: false,
                    app: app.to_string(),
                    suites: None,
                    error: Some(format!("failed to parse sdef XML: {e}")),
                };
            }
            _ => {}
        }
    }

    // Flush last suite
    if let Some(s) = current_suite.take()
        && (!s.classes.is_empty() || !s.commands.is_empty())
    {
        suites.push(s);
    }

    SdefResult {
        ok: true,
        app: app.to_string(),
        suites: Some(suites),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_discovery_does_not_request_dynamic_app_terminology() {
        // This app has only Cocoa metadata; scanning it must not invoke OSA.
        assert_eq!(
            dictionary_info("/System/Applications/TextEdit.app"),
            (true, None)
        );
        let (scriptable, classes) = dictionary_info("/System/Library/CoreServices/Finder.app");
        assert!(scriptable && classes.is_some_and(|count| count > 0));
        assert_eq!(dictionary_info("/nonexistent.app"), (false, None));
    }
}
