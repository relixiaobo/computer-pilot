//! Sdef (Scripting Definition) parser — pure Rust, no osascript.
//! Reads .sdef XML files from app bundles and returns structured JSON.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
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

/// Locate the .sdef file for an app given its bundle path.
fn find_sdef(bundle_path: &str) -> Option<PathBuf> {
    let resources = Path::new(bundle_path).join("Contents/Resources");
    let entries = std::fs::read_dir(&resources).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".sdef") {
            return Some(entry.path());
        }
    }
    None
}

/// Count classes in sdef without full parse (for cu apps).
pub fn count_classes(bundle_path: &str) -> Option<usize> {
    let sdef_path = find_sdef(bundle_path)?;
    let xml = std::fs::read_to_string(&sdef_path).ok()?;
    let mut reader = Reader::from_str(&xml);
    let mut count = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let name = e.name();
                if name.as_ref() == b"class" || name.as_ref() == b"class-extension" {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    Some(count)
}

/// Parse the full sdef and return structured result.
pub fn parse(app: &str, bundle_path: &str) -> SdefResult {
    let sdef_path = match find_sdef(bundle_path) {
        Some(p) => p,
        None => return SdefResult {
            ok: false, app: app.to_string(), suites: None,
            error: Some(format!("{app} is not scriptable (no sdef file)")),
        },
    };

    let xml = match std::fs::read_to_string(&sdef_path) {
        Ok(x) => x,
        Err(e) => return SdefResult {
            ok: false, app: app.to_string(), suites: None,
            error: Some(format!("failed to read sdef: {e}")),
        },
    };

    let mut reader = Reader::from_str(&xml);
    let mut suites = Vec::new();

    // State machine
    enum State { Root, Suite, Class, ClassExt, Command }
    let mut state = State::Root;
    let mut current_suite = Option::<Suite>::None;
    let mut current_class = Option::<Class>::None;
    let mut current_cmd = Option::<SdefCommand>::None;

    fn attr_val(e: &quick_xml::events::BytesStart, key: &[u8]) -> String {
        e.attributes().flatten()
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
                        if let Some(s) = current_suite.take() {
                            if !s.classes.is_empty() || !s.commands.is_empty() {
                                suites.push(s);
                            }
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
                                access: if access.is_empty() { "rw".to_string() } else { access },
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
                                access: if access.is_empty() { "rw".to_string() } else { access },
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
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
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
                        if let Some(s) = current_suite.take() {
                            if !s.classes.is_empty() || !s.commands.is_empty() {
                                suites.push(s);
                            }
                        }
                        state = State::Root;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return SdefResult {
                ok: false, app: app.to_string(), suites: None,
                error: Some(format!("failed to parse sdef XML: {e}")),
            },
            _ => {}
        }
    }

    // Flush last suite
    if let Some(s) = current_suite.take() {
        if !s.classes.is_empty() || !s.commands.is_empty() {
            suites.push(s);
        }
    }

    SdefResult { ok: true, app: app.to_string(), suites: Some(suites), error: None }
}
