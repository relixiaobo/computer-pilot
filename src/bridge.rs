use crate::protocol::{
    self, CAPABILITIES, MACHINE_SCHEMA_VERSION, MAX_COMMAND_CACHE, MAX_MESSAGE_BYTES,
    MAX_RESULT_BYTES, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const DEFAULT_DEADLINE_MS: u64 = 120_000;
const MAX_DEADLINE_MS: u64 = 300_000;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct CachedCommand {
    signature: String,
    result: Value,
}

struct BridgeState {
    initialized: bool,
    granted: HashSet<String>,
    denied_at_launch: HashSet<String>,
    commands: HashMap<String, CachedCommand>,
    command_order: VecDeque<String>,
    connection_id: String,
}

impl BridgeState {
    fn new(denied_at_launch: HashSet<String>) -> Self {
        Self {
            initialized: false,
            granted: HashSet::new(),
            denied_at_launch,
            commands: HashMap::new(),
            command_order: VecDeque::new(),
            connection_id: opaque_id("connection"),
        }
    }

    fn cache(&mut self, command_id: String, signature: String, result: Value) {
        if let Some(cached) = self.commands.get_mut(&command_id) {
            *cached = CachedCommand { signature, result };
            return;
        }
        while self.command_order.len() >= MAX_COMMAND_CACHE {
            if let Some(oldest) = self.command_order.pop_front() {
                self.commands.remove(&oldest);
            }
        }
        self.command_order.push_back(command_id.clone());
        self.commands
            .insert(command_id, CachedCommand { signature, result });
    }
}

#[derive(Debug)]
struct RpcError {
    rpc_code: i64,
    code: &'static str,
    message: String,
    retryable: bool,
    context: Option<Value>,
}

impl RpcError {
    fn invalid(message: impl Into<String>, field: Option<String>) -> Self {
        Self {
            rpc_code: -32602,
            code: "invalid_argument",
            message: message.into(),
            retryable: false,
            context: field.map(|field| json!({"field": field})),
        }
    }

    fn protocol(message: impl Into<String>, context: Option<Value>) -> Self {
        Self {
            rpc_code: -32001,
            code: "protocol_incompatible",
            message: message.into(),
            retryable: false,
            context,
        }
    }

    fn method_not_found(method: &str) -> Self {
        Self {
            rpc_code: -32601,
            code: "method_not_found",
            message: format!("unknown method `{method}`"),
            retryable: false,
            context: Some(json!({"method": method})),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            rpc_code: -32603,
            code: "internal_error",
            message: message.into(),
            retryable: false,
            context: None,
        }
    }

    fn to_value(&self, id: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": self.rpc_code,
                "message": self.message,
                "data": {
                    "code": self.code,
                    "retryable": self.retryable,
                    "context": self.context,
                }
            }
        })
    }
}

pub fn run_stdio(denied: Vec<String>) -> Result<(), String> {
    let supported: HashSet<&str> = CAPABILITIES.iter().copied().collect();
    if let Some(value) = denied
        .iter()
        .find(|value| !supported.contains(value.as_str()))
    {
        return Err(format!("unknown denied capability `{value}`"));
    }
    let mut state = BridgeState::new(denied.into_iter().collect());
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes = read_bounded_line(&mut reader, &mut line, MAX_MESSAGE_BYTES)
            .map_err(|error| format!("failed to read bridge input: {error}"))?;
        if bytes == 0 {
            return Ok(());
        }
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }
        if line.len() > MAX_MESSAGE_BYTES {
            write_response(
                &mut writer,
                &RpcError {
                    rpc_code: -32600,
                    code: "message_too_large",
                    message: format!("protocol message exceeds {MAX_MESSAGE_BYTES} bytes"),
                    retryable: false,
                    context: Some(json!({"maxMessageBytes": MAX_MESSAGE_BYTES})),
                }
                .to_value(Value::Null),
            )?;
            return Err("bridge input exceeded the protocol limit".into());
        }
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(error) => {
                write_response(
                    &mut writer,
                    &RpcError {
                        rpc_code: -32700,
                        code: "parse_error",
                        message: format!("invalid JSON: {error}"),
                        retryable: false,
                        context: None,
                    }
                    .to_value(Value::Null),
                )?;
                return Err("bridge received malformed JSON".into());
            }
        };
        let (response, shutdown) = handle_request(&mut state, request);
        if serialized_len(&response)? > MAX_RESULT_BYTES {
            let id = response.get("id").cloned().unwrap_or(Value::Null);
            if response
                .pointer("/result/command/mutating")
                .and_then(Value::as_bool)
                == Some(true)
            {
                let command_id = response
                    .pointer("/result/command/id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                write_response(
                    &mut writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": unknown_outcome_result(
                            command_id,
                            "mutating command completed but its result exceeded the protocol limit",
                        )
                    }),
                )?;
            } else {
                write_response(
                    &mut writer,
                    &RpcError {
                        rpc_code: -32000,
                        code: "result_too_large",
                        message: format!("protocol result exceeds {MAX_RESULT_BYTES} bytes"),
                        retryable: false,
                        context: Some(json!({"maxResultBytes": MAX_RESULT_BYTES})),
                    }
                    .to_value(id),
                )?;
            }
        } else {
            write_response(&mut writer, &response)?;
        }
        if shutdown {
            return Ok(());
        }
    }
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
    max_message_bytes: usize,
) -> io::Result<usize> {
    reader
        .take((max_message_bytes + 2) as u64)
        .read_until(b'\n', line)
}

fn handle_request(state: &mut BridgeState, request: Value) -> (Value, bool) {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let result = validate_request(&request).and_then(|(method, params)| {
        if method == "initialize" && state.initialized {
            return Err(RpcError {
                rpc_code: -32000,
                code: "already_initialized",
                message: "this bridge connection is already initialized".into(),
                retryable: false,
                context: None,
            });
        }
        if method != "initialize" && !state.initialized {
            return Err(RpcError {
                rpc_code: -32002,
                code: "not_initialized",
                message: "initialize must be the first bridge request".into(),
                retryable: false,
                context: None,
            });
        }
        match method {
            "initialize" => initialize(state, params),
            "tools/list" => {
                Ok(json!({"tools": protocol::available_tool_definitions(&state.granted)}))
            }
            "tools/call" => call_tool(state, params),
            "shutdown" => Ok(json!({"stopped": true})),
            other => Err(RpcError::method_not_found(other)),
        }
    });
    let shutdown =
        request.get("method").and_then(Value::as_str) == Some("shutdown") && result.is_ok();
    match result {
        Ok(result) => (
            json!({"jsonrpc": "2.0", "id": id, "result": result}),
            shutdown,
        ),
        Err(error) => (error.to_value(id), false),
    }
}

fn validate_request(request: &Value) -> Result<(&str, &Value), RpcError> {
    let object = request
        .as_object()
        .ok_or_else(|| RpcError::invalid("JSON-RPC request must be an object", None))?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(RpcError::invalid(
            "jsonrpc must equal `2.0`",
            Some("jsonrpc".into()),
        ));
    }
    match object.get("id") {
        Some(Value::String(_)) | Some(Value::Number(_)) => {}
        _ => {
            return Err(RpcError::invalid(
                "request id must be a string or number",
                Some("id".into()),
            ));
        }
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid("method must be a string", Some("method".into())))?;
    Ok((method, object.get("params").unwrap_or(&Value::Null)))
}

fn initialize(state: &mut BridgeState, params: &Value) -> Result<Value, RpcError> {
    let params = params.as_object().ok_or_else(|| {
        RpcError::invalid("initialize params must be an object", Some("params".into()))
    })?;
    validate_client(params.get("client"))?;
    validate_protocol(params.get("protocol"))?;
    let requested = string_array(params.get("requestedCapabilities"), "requestedCapabilities")?;
    let launch_mode = params
        .get("launchMode")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RpcError::invalid("launchMode must be a string", Some("launchMode".into()))
        })?;
    if !matches!(launch_mode, "embedded" | "one-shot") {
        return Err(RpcError::invalid(
            "launchMode must be `embedded` or `one-shot`",
            Some("launchMode".into()),
        ));
    }

    let supported: HashSet<&str> = CAPABILITIES.iter().copied().collect();
    let mut granted = Vec::new();
    let mut denied = Vec::new();
    let mut unsupported = Vec::new();
    for capability in requested {
        if !supported.contains(capability.as_str()) {
            unsupported.push(capability);
        } else if state.denied_at_launch.contains(&capability) {
            denied.push(capability);
        } else {
            state.granted.insert(capability.clone());
            granted.push(capability);
        }
    }
    state.initialized = true;
    Ok(json!({
        "serviceVersion": crate::VERSION,
        "executableVersion": crate::VERSION,
        "machineSchemaVersion": MACHINE_SCHEMA_VERSION,
        "protocol": {"major": PROTOCOL_MAJOR, "minor": PROTOCOL_MINOR},
        "supportedCapabilities": CAPABILITIES,
        "capabilities": {"granted": granted, "denied": denied, "unsupported": unsupported},
        "connectionId": state.connection_id,
        "limits": {
            "maxMessageBytes": MAX_MESSAGE_BYTES,
            "maxResultBytes": MAX_RESULT_BYTES,
            "maxCommandRecords": MAX_COMMAND_CACHE,
            "maxDeadlineMs": MAX_DEADLINE_MS,
        }
    }))
}

fn validate_client(client: Option<&Value>) -> Result<(), RpcError> {
    let client = client
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid("client must be an object", Some("client".into())))?;
    for field in ["id", "name", "version", "instanceId"] {
        let valid = client
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty() && value.len() <= 256);
        if !valid {
            return Err(RpcError::invalid(
                format!("client.{field} must be a non-empty string up to 256 bytes"),
                Some(format!("client.{field}")),
            ));
        }
    }
    Ok(())
}

fn validate_protocol(protocol: Option<&Value>) -> Result<(), RpcError> {
    let protocol = protocol
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid("protocol must be an object", Some("protocol".into())))?;
    let min = version_pair(protocol.get("min"), "protocol.min")?;
    let max = version_pair(protocol.get("max"), "protocol.max")?;
    let selected = (PROTOCOL_MAJOR, PROTOCOL_MINOR);
    if min > selected || max < selected {
        return Err(RpcError::protocol(
            "no compatible Computer Pilot protocol version",
            Some(json!({
                "supported": {"min": {"major": 1, "minor": 0}, "max": {"major": 1, "minor": 0}},
                "requested": {"min": {"major": min.0, "minor": min.1}, "max": {"major": max.0, "minor": max.1}}
            })),
        ));
    }
    Ok(())
}

fn version_pair(value: Option<&Value>, field: &str) -> Result<(u64, u64), RpcError> {
    let object = value.and_then(Value::as_object).ok_or_else(|| {
        RpcError::invalid(format!("{field} must be an object"), Some(field.into()))
    })?;
    let major = object.get("major").and_then(Value::as_u64).ok_or_else(|| {
        RpcError::invalid(
            format!("{field}.major must be an integer"),
            Some(format!("{field}.major")),
        )
    })?;
    let minor = object.get("minor").and_then(Value::as_u64).ok_or_else(|| {
        RpcError::invalid(
            format!("{field}.minor must be an integer"),
            Some(format!("{field}.minor")),
        )
    })?;
    Ok((major, minor))
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, RpcError> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        RpcError::invalid(format!("{field} must be an array"), Some(field.into()))
    })?;
    values
        .iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                RpcError::invalid(format!("{field} items must be strings"), Some(field.into()))
            })
        })
        .collect()
}

fn call_tool(state: &mut BridgeState, params: &Value) -> Result<Value, RpcError> {
    let params = params.as_object().ok_or_else(|| {
        RpcError::invalid("tools/call params must be an object", Some("params".into()))
    })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid("name must be a string", Some("name".into())))?;
    let tool = protocol::find_tool(name).ok_or_else(|| RpcError {
        rpc_code: -32602,
        code: "tool_not_found",
        message: format!("unknown tool `{name}`"),
        retryable: false,
        context: Some(json!({"tool": name})),
    })?;
    if !state.granted.contains(tool.required_capability) {
        return Err(RpcError {
            rpc_code: -32003,
            code: "capability_denied",
            message: format!(
                "tool `{name}` requires capability `{}`",
                tool.required_capability
            ),
            retryable: false,
            context: Some(json!({"tool": name, "capability": tool.required_capability})),
        });
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let argv = protocol::arguments_to_argv(&tool, &arguments).map_err(|error| RpcError {
        rpc_code: -32602,
        code: error.code,
        message: error.message,
        retryable: false,
        context: error.field.map(|field| json!({"field": field})),
    })?;
    let command_id = match params.get("commandId") {
        Some(Value::String(value)) if !value.is_empty() && value.len() <= 256 => value.clone(),
        Some(_) => {
            return Err(RpcError::invalid(
                "commandId must be a non-empty string up to 256 bytes",
                Some("commandId".into()),
            ));
        }
        None => opaque_id("command"),
    };
    let signature = serde_json::to_string(&json!({"name": name, "arguments": arguments}))
        .map_err(|_| RpcError::internal("failed to encode command identity"))?;
    if let Some(cached) = state.commands.get(&command_id) {
        if cached.signature != signature {
            return Err(RpcError::invalid(
                "commandId is already bound to a different tool call",
                Some("commandId".into()),
            ));
        }
        return Ok(cached.result.clone());
    }
    let deadline_ms = params
        .get("deadlineMs")
        .map(|value| {
            value
                .as_u64()
                .filter(|value| *value > 0 && *value <= MAX_DEADLINE_MS)
                .ok_or_else(|| {
                    RpcError::invalid(
                        format!("deadlineMs must be between 1 and {MAX_DEADLINE_MS}"),
                        Some("deadlineMs".into()),
                    )
                })
        })
        .transpose()?
        .unwrap_or(DEFAULT_DEADLINE_MS);
    let result = execute_tool(&command_id, &tool, &argv, deadline_ms)?;
    state.cache(command_id, signature, result.clone());
    Ok(result)
}

fn execute_tool(
    command_id: &str,
    tool: &protocol::ToolSpec,
    argv: &[String],
    deadline_ms: u64,
) -> Result<Value, RpcError> {
    let capture = TempCapture::new().map_err(|error| {
        RpcError::internal(format!("failed to create private command output: {error}"))
    })?;
    let executable = std::env::current_exe().map_err(|error| {
        RpcError::internal(format!("failed to resolve current executable: {error}"))
    })?;
    let mut child = Command::new(executable)
        .arg("--json")
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::from(capture.stdout_child().map_err(|error| {
            RpcError::internal(format!("failed to prepare child stdout: {error}"))
        })?))
        .stderr(Stdio::from(capture.stderr_child().map_err(|error| {
            RpcError::internal(format!("failed to prepare child stderr: {error}"))
        })?))
        .spawn()
        .map_err(|error| RpcError::internal(format!("failed to launch cu command: {error}")))?;

    let deadline = Instant::now() + Duration::from_millis(deadline_ms);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return post_launch_failure(
                    command_id,
                    tool,
                    format!("failed to wait for cu command: {error}"),
                );
            }
        }
    };
    let stdout = match capture.read_stdout() {
        Ok(stdout) => stdout,
        Err(error) => {
            return post_launch_failure(
                command_id,
                tool,
                format!("failed to read command stdout: {error}"),
            );
        }
    };
    let stderr = match capture.read_stderr() {
        Ok(stderr) => stderr,
        Err(error) => {
            return post_launch_failure(
                command_id,
                tool,
                format!("failed to read command stderr: {error}"),
            );
        }
    };
    if stdout.len() > MAX_RESULT_BYTES || stderr.len() > MAX_RESULT_BYTES {
        if tool.mutating {
            return post_launch_failure(
                command_id,
                tool,
                "mutating command completed but its output exceeded the protocol limit",
            );
        }
        return Ok(command_error(
            command_id,
            tool,
            "failed",
            json!({
                "schema_version": MACHINE_SCHEMA_VERSION,
                "ok": false,
                "code": "result_too_large",
                "error": format!("command output exceeds {MAX_RESULT_BYTES} bytes"),
                "retryable": false,
            }),
        ));
    }
    let Some(status) = status else {
        let code = if tool.mutating {
            "unknown_outcome"
        } else {
            "command_expired"
        };
        let message = if tool.mutating {
            "mutating command exceeded its deadline after launch; inspect current UI state before continuing"
        } else {
            "command exceeded its deadline"
        };
        return Ok(command_error(
            command_id,
            tool,
            code,
            json!({
                "schema_version": MACHINE_SCHEMA_VERSION,
                "ok": false,
                "code": code,
                "error": message,
                "retryable": !tool.mutating,
                "diagnostics": {"deadline_ms": deadline_ms}
            }),
        ));
    };
    decode_child_result(command_id, tool, status, &stdout, &stderr)
}

fn decode_child_result(
    command_id: &str,
    tool: &protocol::ToolSpec,
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Value, RpcError> {
    if status.success() {
        let result: Value = match serde_json::from_slice(trim_ascii(stdout)) {
            Ok(result) => result,
            Err(error) => {
                return post_launch_failure(
                    command_id,
                    tool,
                    format!("cu command returned invalid machine JSON: {error}"),
                );
            }
        };
        if !result.is_object() {
            return post_launch_failure(
                command_id,
                tool,
                "cu command returned a non-object machine result",
            );
        }
        return Ok(json!({
            "command": {"id": command_id, "status": "completed", "mutating": tool.mutating},
            "result": result
        }));
    }
    let error = match serde_json::from_slice::<Value>(trim_ascii(stderr)) {
        Ok(error) if error.is_object() => error,
        Ok(_) if tool.mutating => {
            return post_launch_failure(
                command_id,
                tool,
                "mutating cu command returned a non-object machine error",
            );
        }
        Err(error) if tool.mutating => {
            return post_launch_failure(
                command_id,
                tool,
                format!("mutating cu command returned invalid machine JSON: {error}"),
            );
        }
        _ => {
            let message = String::from_utf8_lossy(trim_ascii(stderr));
            json!({
                "schema_version": MACHINE_SCHEMA_VERSION,
                "ok": false,
                "code": "command_failed",
                "error": bounded(&message, 4096),
                "retryable": false,
            })
        }
    };
    Ok(command_error(command_id, tool, "failed", error))
}

fn post_launch_failure(
    command_id: &str,
    tool: &protocol::ToolSpec,
    detail: impl Into<String>,
) -> Result<Value, RpcError> {
    let detail = detail.into();
    if tool.mutating {
        Ok(unknown_outcome_result(command_id, &detail))
    } else {
        Err(RpcError::internal(detail))
    }
}

fn unknown_outcome_result(command_id: &str, reason: &str) -> Value {
    json!({
        "command": {"id": command_id, "status": "unknown_outcome", "mutating": true},
        "error": {
            "schema_version": MACHINE_SCHEMA_VERSION,
            "ok": false,
            "code": "unknown_outcome",
            "error": "mutating command was launched but its terminal result could not be established; inspect current UI state before continuing",
            "retryable": false,
            "diagnostics": {"reason": bounded(reason, 1024)}
        }
    })
}

fn command_error(command_id: &str, tool: &protocol::ToolSpec, status: &str, error: Value) -> Value {
    json!({
        "command": {"id": command_id, "status": status, "mutating": tool.mutating},
        "error": error
    })
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|index| index + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn bounded(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn write_response(writer: &mut impl Write, response: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, response)
        .map_err(|error| format!("failed to encode bridge response: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|error| format!("failed to write bridge response: {error}"))
}

fn serialized_len(value: &Value) -> Result<usize, String> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| format!("failed to encode bridge response: {error}"))
}

fn opaque_id(prefix: &str) -> String {
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}:{:x}:{nanos:x}:{sequence:x}", std::process::id())
}

struct TempCapture {
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    stdout: File,
    stderr: File,
}

impl TempCapture {
    fn new() -> io::Result<Self> {
        let id = opaque_id("stdio").replace(':', "-");
        let stdout_path = std::env::temp_dir().join(format!("cu-bridge-{id}.stdout"));
        let stderr_path = std::env::temp_dir().join(format!("cu-bridge-{id}.stderr"));
        let stdout = private_file(&stdout_path)?;
        let stderr = match private_file(&stderr_path) {
            Ok(file) => file,
            Err(error) => {
                let _ = std::fs::remove_file(&stdout_path);
                return Err(error);
            }
        };
        Ok(Self {
            stdout_path,
            stderr_path,
            stdout,
            stderr,
        })
    }

    fn stdout_child(&self) -> io::Result<File> {
        self.stdout.try_clone()
    }

    fn stderr_child(&self) -> io::Result<File> {
        self.stderr.try_clone()
    }

    fn read_stdout(&self) -> io::Result<Vec<u8>> {
        read_file(&self.stdout_path)
    }

    fn read_stderr(&self) -> io::Result<Vec<u8>> {
        read_file(&self.stderr_path)
    }
}

impl Drop for TempCapture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.stdout_path);
        let _ = std::fs::remove_file(&self.stderr_path);
    }
}

fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_trimming_is_safe_for_empty_and_json_bytes() {
        assert_eq!(trim_ascii(b"  \n"), b"");
        assert_eq!(trim_ascii(b" \n {\"ok\":true}\r\n"), b"{\"ok\":true}");
    }

    #[test]
    fn input_is_bounded_before_a_newline_is_found() {
        let mut reader = io::Cursor::new(b"123456789\n");
        let mut line = Vec::new();
        let bytes = read_bounded_line(&mut reader, &mut line, 4).unwrap();
        assert_eq!(bytes, 6);
        assert_eq!(line, b"123456");
    }

    #[test]
    fn denied_capability_filters_manifest() {
        let granted: HashSet<String> = ["desktop.discover".to_string()].into_iter().collect();
        let tools = protocol::available_tool_definitions(&granted);
        assert!(tools.iter().any(|tool| tool["name"] == "computer.setup"));
        assert!(!tools.iter().any(|tool| tool["name"] == "computer.click"));
    }

    #[test]
    fn a_connection_cannot_reinitialize_to_expand_capabilities() {
        let mut state = BridgeState::new(HashSet::new());
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "client": {"id": "test", "name": "Test", "version": "1", "instanceId": "one"},
                "protocol": {"min": {"major": 1, "minor": 0}, "max": {"major": 1, "minor": 0}},
                "requestedCapabilities": ["desktop.discover"],
                "launchMode": "embedded"
            }
        });
        let (first, _) = handle_request(&mut state, initialize.clone());
        assert!(first.get("result").is_some());
        let (second, _) = handle_request(&mut state, initialize);
        assert_eq!(second["error"]["data"]["code"], "already_initialized");
        assert_eq!(
            state.granted,
            HashSet::from(["desktop.discover".to_string()])
        );
    }

    #[test]
    fn mutating_post_launch_failures_are_never_retryable() {
        let tool = protocol::find_tool("computer.click").unwrap();
        let result = post_launch_failure("command:test", &tool, "lost child output").unwrap();
        assert_eq!(result["command"]["status"], "unknown_outcome");
        assert_eq!(result["error"]["code"], "unknown_outcome");
        assert_eq!(result["error"]["retryable"], false);
    }

    #[test]
    fn read_only_post_launch_failures_remain_internal_errors() {
        let tool = protocol::find_tool("computer.examples").unwrap();
        let error = post_launch_failure("command:test", &tool, "lost child output").unwrap_err();
        assert_eq!(error.code, "internal_error");
    }
}
