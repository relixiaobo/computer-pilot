use crate::Cli;
use clap::{Arg, ArgAction, Command, CommandFactory};
use serde_json::{Map, Value, json};
use std::any::TypeId;
use std::collections::HashSet;

pub const MACHINE_SCHEMA_VERSION: &str = "1.0";
pub const PROTOCOL_MAJOR: u64 = 1;
pub const PROTOCOL_MINOR: u64 = 0;
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_COMMAND_CACHE: usize = 256;

pub const CAPABILITIES: &[&str] = &[
    "desktop.discover",
    "desktop.observe",
    "desktop.capture",
    "desktop.input",
    "desktop.pointer",
    "desktop.window",
    "desktop.app",
    "desktop.script",
    "desktop.defaults",
];

#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub command: String,
    pub required_capability: &'static str,
    pub mutating: bool,
    pub definition: Value,
    command_definition: Command,
}

#[derive(Debug)]
pub struct ProtocolError {
    pub code: &'static str,
    pub message: String,
    pub field: Option<String>,
}

impl ProtocolError {
    fn invalid(message: impl Into<String>, field: impl Into<Option<String>>) -> Self {
        Self {
            code: "invalid_argument",
            message: message.into(),
            field: field.into(),
        }
    }
}

pub fn tool_specs() -> Vec<ToolSpec> {
    Cli::command()
        .get_subcommands()
        .filter(|command| command.get_name() != "bridge")
        .map(tool_spec)
        .collect()
}

pub fn available_tool_definitions(granted: &HashSet<String>) -> Vec<Value> {
    tool_specs()
        .into_iter()
        .filter(|tool| granted.contains(tool.required_capability))
        .map(|tool| tool.definition)
        .collect()
}

pub fn find_tool(name: &str) -> Option<ToolSpec> {
    tool_specs().into_iter().find(|tool| tool.name == name)
}

pub fn arguments_to_argv(tool: &ToolSpec, arguments: &Value) -> Result<Vec<String>, ProtocolError> {
    let object = arguments.as_object().ok_or_else(|| {
        ProtocolError::invalid(
            "tool arguments must be a JSON object",
            Some("arguments".into()),
        )
    })?;
    let known: HashSet<&str> = tool
        .command_definition
        .get_arguments()
        .filter(|arg| exposed_arg(arg))
        .map(|arg| arg.get_id().as_str())
        .collect();

    if let Some(unknown) = object.keys().find(|key| !known.contains(key.as_str())) {
        return Err(ProtocolError::invalid(
            format!("unknown argument `{unknown}` for {}", tool.name),
            Some(format!("arguments.{unknown}")),
        ));
    }

    let mut argv = vec![tool.command.clone()];
    let mut positionals: Vec<&Arg> = tool
        .command_definition
        .get_arguments()
        .filter(|arg| exposed_arg(arg) && arg.is_positional())
        .collect();
    positionals.sort_by_key(|arg| arg.get_index().unwrap_or(usize::MAX));

    for arg in positionals {
        append_positional(&mut argv, arg, object.get(arg.get_id().as_str()))?;
    }
    for arg in tool
        .command_definition
        .get_arguments()
        .filter(|arg| exposed_arg(arg) && !arg.is_positional())
    {
        append_option(&mut argv, arg, object.get(arg.get_id().as_str()))?;
    }
    Ok(argv)
}

fn tool_spec(command: &Command) -> ToolSpec {
    let command_name = command.get_name().to_string();
    let name = format!("computer.{}", command_name.replace('-', "_"));
    let required_capability = required_capability(&command_name);
    let mutating = is_mutating(&command_name);
    let input_schema = input_schema(command);
    let description = command
        .get_long_about()
        .or_else(|| command.get_about())
        .map(ToString::to_string)
        .unwrap_or_else(|| command_name.clone());
    let sensitivity = sensitivity(&command_name);
    let artifact_kinds = artifact_kinds(&command_name);
    let definition = json!({
        "name": name,
        "title": command_name.replace('-', " "),
        "description": description,
        "command": command_name,
        "inputSchema": input_schema,
        "outputSchema": {
            "type": "object",
            "properties": {
                "schema_version": {"type": "string", "const": MACHINE_SCHEMA_VERSION},
                "ok": {"type": "boolean"}
            },
            "required": ["schema_version", "ok"],
            "additionalProperties": true
        },
        "requiredCapabilities": [required_capability],
        "mutating": mutating,
        "idempotency": if mutating { "caller_keyed" } else { "safe" },
        "sensitivity": sensitivity,
        "artifactKinds": artifact_kinds,
    });
    ToolSpec {
        name: format!("computer.{}", command_name.replace('-', "_")),
        command: command_name,
        required_capability,
        mutating,
        definition,
        command_definition: command.clone(),
    }
}

fn input_schema(command: &Command) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for arg in command.get_arguments().filter(|arg| exposed_arg(arg)) {
        let id = arg.get_id().as_str().to_string();
        properties.insert(id.clone(), schema_for_arg(arg));
        if arg.is_required_set() {
            required.push(Value::String(id));
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn schema_for_arg(arg: &Arg) -> Value {
    let action = arg.get_action();
    let mut schema = match action {
        ArgAction::SetTrue | ArgAction::SetFalse => json!({"type": "boolean"}),
        ArgAction::Count => json!({"type": "integer", "minimum": 0, "maximum": 32}),
        _ => value_schema(arg),
    };
    if let Some(description) = arg
        .get_long_help()
        .or_else(|| arg.get_help())
        .map(ToString::to_string)
    {
        schema["description"] = Value::String(description);
    }
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    if defaults.len() == 1 {
        schema["default"] = typed_default(&schema, &defaults[0]);
    } else if !defaults.is_empty() {
        schema["default"] = defaults
            .iter()
            .map(|value| typed_default(&schema["items"], value))
            .collect::<Vec<_>>()
            .into();
    }
    schema
}

fn typed_default(schema: &Value, value: &str) -> Value {
    match schema.get("type").and_then(Value::as_str) {
        Some("integer") => value
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(value.into())),
        Some("number") => value
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.into())),
        Some("boolean") => value
            .parse::<bool>()
            .map(Value::Bool)
            .unwrap_or_else(|_| Value::String(value.into())),
        _ => Value::String(value.into()),
    }
}

fn value_schema(arg: &Arg) -> Value {
    let parser_id = arg.get_value_parser().type_id();
    let mut item = if parser_id == TypeId::of::<usize>()
        || parser_id == TypeId::of::<u64>()
        || parser_id == TypeId::of::<u32>()
        || parser_id == TypeId::of::<i64>()
        || parser_id == TypeId::of::<i32>()
    {
        json!({"type": "integer"})
    } else if parser_id == TypeId::of::<f64>() || parser_id == TypeId::of::<f32>() {
        json!({"type": "number"})
    } else {
        json!({"type": "string"})
    };
    if let Some(values) = arg.get_value_parser().possible_values() {
        let values: Vec<Value> = values
            .filter(|value| !value.is_hide_set())
            .map(|value| Value::String(value.get_name().to_string()))
            .collect();
        if !values.is_empty() {
            item["enum"] = Value::Array(values);
        }
    }
    let range = arg.get_num_args().unwrap_or_else(|| 1.into());
    let multiple = matches!(arg.get_action(), ArgAction::Append) || range.max_values() > 1;
    if multiple {
        let mut array = json!({"type": "array", "items": item});
        if range.min_values() > 0 {
            array["minItems"] = range.min_values().into();
        }
        if range.max_values() != usize::MAX {
            array["maxItems"] = range.max_values().into();
        }
        array
    } else {
        item
    }
}

fn exposed_arg(arg: &Arg) -> bool {
    !arg.is_hide_set()
        && !matches!(
            arg.get_action(),
            ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
        )
}

fn append_positional(
    argv: &mut Vec<String>,
    arg: &Arg,
    value: Option<&Value>,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        if arg.is_required_set() {
            return Err(missing(arg));
        }
        return Ok(());
    };
    append_values(argv, arg, value)
}

fn append_option(
    argv: &mut Vec<String>,
    arg: &Arg,
    value: Option<&Value>,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        if arg.is_required_set() {
            return Err(missing(arg));
        }
        return Ok(());
    };
    let flag = arg
        .get_long()
        .map(|long| format!("--{long}"))
        .or_else(|| arg.get_short().map(|short| format!("-{short}")))
        .ok_or_else(|| ProtocolError::invalid("argument has no CLI spelling", None))?;
    match arg.get_action() {
        ArgAction::SetTrue => match value.as_bool() {
            Some(true) => argv.push(flag),
            Some(false) => {}
            None => return Err(type_error(arg, "boolean")),
        },
        ArgAction::SetFalse => match value.as_bool() {
            Some(false) => argv.push(flag),
            Some(true) => {}
            None => return Err(type_error(arg, "boolean")),
        },
        ArgAction::Count => {
            let count = value.as_u64().ok_or_else(|| type_error(arg, "integer"))?;
            if count > 32 {
                return Err(ProtocolError::invalid(
                    format!("argument `{}` must be at most 32", arg.get_id()),
                    Some(format!("arguments.{}", arg.get_id())),
                ));
            }
            for _ in 0..count {
                argv.push(flag.clone());
            }
        }
        _ => {
            argv.push(flag);
            append_values(argv, arg, value)?;
        }
    }
    Ok(())
}

fn append_values(argv: &mut Vec<String>, arg: &Arg, value: &Value) -> Result<(), ProtocolError> {
    let range = arg.get_num_args().unwrap_or_else(|| 1.into());
    let multiple = matches!(arg.get_action(), ArgAction::Append) || range.max_values() > 1;
    if multiple {
        let values = value.as_array().ok_or_else(|| type_error(arg, "array"))?;
        if values.len() < range.min_values()
            || (range.max_values() != usize::MAX && values.len() > range.max_values())
        {
            return Err(ProtocolError::invalid(
                format!("argument `{}` has the wrong number of values", arg.get_id()),
                Some(format!("arguments.{}", arg.get_id())),
            ));
        }
        for value in values {
            argv.push(scalar_string(arg, value)?);
        }
    } else {
        argv.push(scalar_string(arg, value)?);
    }
    Ok(())
}

fn scalar_string(arg: &Arg, value: &Value) -> Result<String, ProtocolError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(type_error(arg, "string or number")),
    }
}

fn missing(arg: &Arg) -> ProtocolError {
    ProtocolError::invalid(
        format!("missing required argument `{}`", arg.get_id()),
        Some(format!("arguments.{}", arg.get_id())),
    )
}

fn type_error(arg: &Arg, expected: &str) -> ProtocolError {
    ProtocolError::invalid(
        format!("argument `{}` must be {expected}", arg.get_id()),
        Some(format!("arguments.{}", arg.get_id())),
    )
}

fn required_capability(command: &str) -> &'static str {
    match command {
        "setup" | "apps" | "menu" | "sdef" | "examples" => "desktop.discover",
        "snapshot" | "find" | "nearest" | "observe-region" | "ocr" | "wait" | "why" => {
            "desktop.observe"
        }
        "screenshot" | "state" => "desktop.capture",
        "click" | "type" | "key" | "set-value" | "perform" => "desktop.input",
        "scroll" | "hover" | "drag" => "desktop.pointer",
        "window" => "desktop.window",
        "launch" | "warm" => "desktop.app",
        "tell" => "desktop.script",
        "defaults" => "desktop.defaults",
        _ => "desktop.observe",
    }
}

fn is_mutating(command: &str) -> bool {
    matches!(
        command,
        "click"
            | "type"
            | "key"
            | "set-value"
            | "perform"
            | "scroll"
            | "hover"
            | "drag"
            | "window"
            | "launch"
            | "warm"
            | "tell"
            | "defaults"
    )
}

fn sensitivity(command: &str) -> Value {
    let input: Vec<&str> = match command {
        "type" | "set-value" | "tell" | "defaults" => vec!["user_input"],
        _ => Vec::new(),
    };
    let output: Vec<&str> = match command {
        "snapshot" | "state" | "find" | "nearest" | "observe-region" | "ocr" | "screenshot"
        | "wait" | "menu" | "sdef" | "apps" => vec!["desktop_data"],
        _ => Vec::new(),
    };
    json!({"input": input, "output": output})
}

fn artifact_kinds(command: &str) -> Vec<&'static str> {
    match command {
        "snapshot" | "state" | "screenshot" => vec!["image/png"],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_every_automation_command_and_no_bridge() {
        let tools = tool_specs();
        assert_eq!(tools.len(), 27);
        assert!(tools.iter().any(|tool| tool.name == "computer.snapshot"));
        assert!(!tools.iter().any(|tool| tool.name == "computer.bridge"));
    }

    #[test]
    fn structured_arguments_become_strict_argv() {
        let tool = find_tool("computer.snapshot").unwrap();
        let argv = arguments_to_argv(
            &tool,
            &json!({"app": "Finder", "limit": 12, "with_screenshot": true}),
        )
        .unwrap();
        assert_eq!(
            argv,
            vec!["snapshot", "Finder", "--limit", "12", "--with-screenshot"]
        );
    }

    #[test]
    fn unknown_arguments_are_rejected_before_cli_dispatch() {
        let tool = find_tool("computer.setup").unwrap();
        let error = arguments_to_argv(&tool, &json!({"shell": "rm -rf /"})).unwrap_err();
        assert_eq!(error.code, "invalid_argument");
        assert_eq!(error.field.as_deref(), Some("arguments.shell"));
    }

    #[test]
    fn numeric_defaults_keep_their_json_schema_type() {
        let tool = find_tool("computer.snapshot").unwrap();
        assert_eq!(
            tool.definition["inputSchema"]["properties"]["limit"]["type"],
            "integer"
        );
        assert_eq!(
            tool.definition["inputSchema"]["properties"]["limit"]["default"],
            50
        );
    }
}
