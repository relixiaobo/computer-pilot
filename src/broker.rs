//! Private per-user command broker.
//!
//! This transport is deliberately internal. Agent hosts invoke the public CLI;
//! short-lived CLI processes use this broker for command recovery and desktop
//! coordination.

use crate::error::{CuError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTERNAL_PROTOCOL: u32 = 2;
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const MAX_RECORDS: usize = 2048;
const MAX_OBSERVATIONS: usize = 512;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const START_WAIT_MS: u64 = 5_000;
const CHILD_MARKER: &str = "COMPUTER_PILOT_BROKER_CHILD";
const EXPECTED_OBSERVATION: &str = "COMPUTER_PILOT_EXPECTED_OBSERVATION";
const INTERNAL_CLIENT_KEY: &str = "COMPUTER_PILOT_INTERNAL_CLIENT_KEY";
const OBSERVATION_FILE: &str = "COMPUTER_PILOT_OBSERVATION_FILE";
const OBSERVATION_TTL_MS: u64 = 5 * 60_000;
const OUTPUT_DIR: &str = "COMPUTER_PILOT_OUTPUT_DIR";
const TEST_FRONTMOST_OVERRIDE: &str = "CU_TEST_FRONTMOST_OVERRIDE";

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

unsafe extern "C" {
    fn setsid() -> i32;
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub client_key: String,
    pub request_id: Option<String>,
    pub timeout_ms: u64,
    pub command: String,
    pub argv: Vec<String>,
    pub mutating: bool,
    pub resource: Option<String>,
    pub desktop_lock: bool,
    pub ref_id: Option<usize>,
    pub observation_id: Option<String>,
    pub output_dir: Option<String>,
    pub test_frontmost_override: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ObservedElement {
    #[serde(rename = "ref")]
    pub ref_id: usize,
    pub role: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(rename = "axPath", default)]
    pub ax_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ObservationExpectation {
    pub observation_id: String,
    pub client_key: String,
    pub pid: i32,
    pub bundle_id: String,
    pub window_id: u32,
    pub ax_generation: u64,
    pub limit: usize,
    pub target: ObservedElement,
    pub elements: Vec<ObservedElement>,
}

#[derive(Clone)]
struct ObservationRecord {
    id: String,
    client_key: String,
    resource: String,
    pid: i32,
    bundle_id: String,
    window_id: u32,
    ax_generation: u64,
    limit: usize,
    elements: Vec<ObservedElement>,
    created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandRecord {
    pub id: String,
    pub client_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub command: String,
    pub mutating: bool,
    pub status: String,
    pub accepted_at_ms: u64,
    pub deadline_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatched_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub desktop_lock: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancellation_requested: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunResult {
    pub command: CommandRecord,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub struct BrokerError {
    pub code: String,
    pub error: String,
    pub retryable: bool,
    pub command_id: Option<String>,
}

impl BrokerError {
    fn internal(error: impl Into<String>) -> Self {
        Self {
            code: "internal_error".into(),
            error: error.into(),
            retryable: false,
            command_id: None,
        }
    }

    pub fn into_cu_error(self) -> CuError {
        let code = match self.code.as_str() {
            "invalid_argument" => ErrorCode::InvalidArgument,
            "observation_required" => ErrorCode::ObservationRequired,
            "observation_not_found" => ErrorCode::ObservationNotFound,
            "stale_observation" => ErrorCode::StaleObservation,
            "request_id_conflict" => ErrorCode::RequestIdConflict,
            "command_in_progress" => ErrorCode::CommandInProgress,
            "command_not_found" => ErrorCode::CommandNotFound,
            "command_cancelled" => ErrorCode::CommandCancelled,
            "command_expired" => ErrorCode::CommandExpired,
            "unknown_outcome" => ErrorCode::UnknownOutcome,
            "target_busy" => ErrorCode::TargetBusy,
            "permission_denied" => ErrorCode::PermissionDenied,
            "app_not_found" => ErrorCode::AppNotFound,
            "window_not_found" => ErrorCode::WindowNotFound,
            "capture_protected" => ErrorCode::CaptureProtected,
            "verification_failed" => ErrorCode::VerificationFailed,
            "command_failed" => ErrorCode::CommandFailed,
            _ => ErrorCode::InternalError,
        };
        let mut error = CuError::msg(self.error)
            .with_code(code)
            .retryable(self.retryable);
        if let Some(command_id) = self.command_id {
            error = error.with_diagnostics(serde_json::json!({"command_id": command_id}));
        }
        error
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrokerStatus {
    pub running: bool,
    pub pid: u32,
    pub protocol: u32,
    pub command_count: usize,
    pub active_count: usize,
    pub uncertain_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Ping {
        token: String,
    },
    Run {
        token: String,
        executable: PathBuf,
        client_key: String,
        request_id: Option<String>,
        timeout_ms: u64,
        command: String,
        argv: Vec<String>,
        mutating: bool,
        resource: Option<String>,
        desktop_lock: bool,
        ref_id: Option<usize>,
        observation_id: Option<String>,
        output_dir: Option<String>,
        #[serde(default)]
        test_frontmost_override: Option<String>,
    },
    Status {
        token: String,
        client_key: String,
    },
    Commands {
        token: String,
        client_key: String,
        limit: usize,
        statuses: Vec<String>,
    },
    Command {
        token: String,
        client_key: String,
        command_id: String,
    },
    Cancel {
        token: String,
        client_key: String,
        command_id: String,
    },
    StopIfIdle {
        token: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Pong {
        #[serde(default)]
        protocol: u32,
        #[serde(default)]
        pid: u32,
        #[serde(default)]
        version: String,
    },
    Run {
        result: RunResult,
    },
    Status {
        status: BrokerStatus,
    },
    Commands {
        commands: Vec<CommandRecord>,
    },
    Command {
        command: CommandRecord,
    },
    Stopping,
    Error {
        code: String,
        error: String,
        retryable: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        command_id: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredRecord {
    descriptor: CommandRecord,
    fingerprint: String,
    stdout: String,
    stderr: String,
}

#[derive(Default)]
struct BrokerState {
    records: HashMap<String, StoredRecord>,
    order: Vec<String>,
    requests: HashMap<(String, String), String>,
    readers: HashMap<String, usize>,
    writers: HashSet<String>,
    active_mutations: usize,
    desktop_active: bool,
    child_pids: HashMap<String, u32>,
    observations: HashMap<String, ObservationRecord>,
    latest_observations: HashMap<(String, String), String>,
    draining: bool,
}

type SharedState = Arc<(Mutex<BrokerState>, Condvar)>;

fn is_false(value: &bool) -> bool {
    !*value
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

pub(crate) fn runtime_home() -> PathBuf {
    std::env::var_os("COMPUTER_PILOT_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("Library/Application Support/computer-pilot"))
                .unwrap_or_else(|| std::env::temp_dir().join("computer-pilot"))
        })
}

fn socket_path() -> PathBuf {
    runtime_home().join("broker.sock")
}

fn token_path() -> PathBuf {
    runtime_home().join("broker.token")
}

fn start_lock_path() -> PathBuf {
    runtime_home().join("broker.start.lock")
}

fn commands_dir() -> PathBuf {
    runtime_home().join("commands")
}

fn prepare_home() -> Result<(), String> {
    let home = runtime_home();
    if let Ok(metadata) = fs::symlink_metadata(&home)
        && metadata.file_type().is_symlink()
    {
        return Err("private Broker home must not be a symlink".into());
    }
    fs::create_dir_all(&home).map_err(|error| format!("failed to create Broker home: {error}"))?;
    fs::set_permissions(&home, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to secure Broker home: {error}"))?;
    prepare_commands_dir()
}

fn prepare_commands_dir() -> Result<(), String> {
    let path = commands_dir();
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && metadata.file_type().is_symlink()
    {
        return Err("private Broker commands directory must not be a symlink".into());
    }
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create Broker commands directory: {error}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to secure Broker commands directory: {error}"))
}

fn record_path(command_id: &str) -> PathBuf {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut filename = String::with_capacity(command_id.len().saturating_mul(2) + 5);
    for byte in command_id.bytes() {
        filename.push(HEX[(byte >> 4) as usize] as char);
        filename.push(HEX[(byte & 0x0f) as usize] as char);
    }
    filename.push_str(".json");
    commands_dir().join(filename)
}

fn persist_record(record: &StoredRecord) -> Result<(), String> {
    prepare_commands_dir()?;
    let encoded = serde_json::to_vec(record)
        .map_err(|error| format!("failed to encode command record: {error}"))?;
    let target = record_path(&record.descriptor.id);
    let temporary = commands_dir().join(format!(
        ".record-{}-{}.tmp",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| format!("failed to create command record: {error}"))?;
        file.write_all(&encoded)
            .map_err(|error| format!("failed to write command record: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync command record: {error}"))?;
        fs::rename(&temporary, &target)
            .map_err(|error| format!("failed to publish command record: {error}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn remove_persisted_record(command_id: &str) {
    let _ = fs::remove_file(record_path(command_id));
}

fn load_state() -> Result<BrokerState, String> {
    prepare_commands_dir()?;
    let mut stored = Vec::new();
    for entry in fs::read_dir(commands_dir())
        .map_err(|error| format!("failed to read Broker commands directory: {error}"))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            _ => continue,
        };
        if metadata.len()
            > (MAX_OUTPUT_BYTES as u64)
                .saturating_mul(2)
                .saturating_add(1_048_576)
        {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(record) = serde_json::from_slice::<StoredRecord>(&bytes) else {
            continue;
        };
        if record.descriptor.id.is_empty() || record.descriptor.client_key.is_empty() {
            continue;
        }
        stored.push(record);
    }
    stored.sort_by_key(|record| record.descriptor.accepted_at_ms);
    if stored.len() > MAX_RECORDS {
        let pruned = stored.len() - MAX_RECORDS;
        for record in stored.drain(..pruned) {
            remove_persisted_record(&record.descriptor.id);
        }
    }

    let mut state = BrokerState::default();
    for mut record in stored {
        if !is_terminal(&record.descriptor.status) {
            record.descriptor.status = "unknown_outcome".into();
            record.descriptor.completed_at_ms = Some(now_ms());
            record.descriptor.exit_code = Some(1);
            record.stderr = serde_json::json!({
                "schema_version": crate::MACHINE_SCHEMA_VERSION,
                "ok": false,
                "code": "unknown_outcome",
                "error": "private Broker restarted after command acceptance; inspect current UI state before retrying",
                "retryable": false,
                "command_id": record.descriptor.id,
            })
            .to_string();
            persist_record(&record)?;
        }
        let command_id = record.descriptor.id.clone();
        if state.records.contains_key(&command_id) {
            continue;
        }
        if let Some(request_id) = &record.descriptor.request_id {
            state.requests.insert(
                (record.descriptor.client_key.clone(), request_id.clone()),
                command_id.clone(),
            );
        }
        state.order.push(command_id.clone());
        state.records.insert(command_id, record);
    }
    Ok(state)
}

fn create_token_if_missing() -> Result<String, String> {
    prepare_home()?;
    let path = token_path();
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let token = format!(
        "{:x}{:x}{:x}",
        now_ms(),
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("failed to create Broker token: {error}"))?;
    file.write_all(token.as_bytes())
        .map_err(|error| format!("failed to write Broker token: {error}"))?;
    Ok(token)
}

fn read_token() -> Result<String, String> {
    fs::read_to_string(token_path())
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("failed to read Broker token: {error}"))
}

fn error_response(code: &str, error: impl Into<String>, retryable: bool) -> Response {
    Response::Error {
        code: code.into(),
        error: error.into(),
        retryable,
        command_id: None,
    }
}

fn send_request(request: &Request, timeout_ms: u64) -> Result<Response, String> {
    let mut stream = UnixStream::connect(socket_path())
        .map_err(|error| format!("cannot reach private Broker: {error}"))?;
    let timeout = Duration::from_millis(timeout_ms.saturating_add(5_000));
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("failed to set Broker read timeout: {error}"))?;
    let mut payload = serde_json::to_vec(request)
        .map_err(|error| format!("failed to encode Broker request: {error}"))?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .map_err(|error| format!("failed to write Broker request: {error}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|error| format!("failed to read Broker response: {error}"))?;
    if line.is_empty() {
        return Err("private Broker closed without a response".into());
    }
    serde_json::from_str(&line)
        .map_err(|error| format!("private Broker returned an invalid response: {error}"))
}

fn ping(token: &str) -> Option<(u32, String, u32)> {
    match send_request(
        &Request::Ping {
            token: token.into(),
        },
        1_000,
    ) {
        Ok(Response::Pong {
            protocol,
            version,
            pid,
        }) => Some((protocol, version, pid)),
        _ => None,
    }
}

fn ensure_running() -> Result<String, BrokerError> {
    prepare_home().map_err(BrokerError::internal)?;
    let token = create_token_if_missing()
        .or_else(|_| read_token())
        .map_err(BrokerError::internal)?;
    if let Some((protocol, version, _pid)) = ping(&token)
        && protocol == INTERNAL_PROTOCOL
        && version == crate::VERSION
    {
        return Ok(token);
    }

    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(start_lock_path());
    let owns_start_lock = lock.is_ok();
    if owns_start_lock {
        if let Some((protocol, version, pid)) = ping(&token) {
            if protocol == INTERNAL_PROTOCOL && version == crate::VERSION {
                let _ = fs::remove_file(start_lock_path());
                return Ok(token);
            }
            if let Err(error) = stop_incompatible_broker(&token, protocol, &version, pid) {
                let _ = fs::remove_file(start_lock_path());
                return Err(error);
            }
        }
        let _ = fs::remove_file(socket_path());
        let executable = std::env::current_exe().map_err(|error| {
            BrokerError::internal(format!("failed to locate cu executable: {error}"))
        })?;
        let mut command = Command::new(executable);
        command
            .arg("__broker")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(|| {
                if setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.spawn().map_err(|error| {
            BrokerError::internal(format!("failed to start private Broker: {error}"))
        })?;
    }

    let deadline = now_ms().saturating_add(START_WAIT_MS);
    while now_ms() < deadline {
        if ping(&token).is_some_and(|(protocol, version, _)| {
            protocol == INTERNAL_PROTOCOL && version == crate::VERSION
        }) {
            if owns_start_lock {
                let _ = fs::remove_file(start_lock_path());
            }
            return Ok(token);
        }
        thread::sleep(Duration::from_millis(25));
    }
    if owns_start_lock {
        let _ = fs::remove_file(start_lock_path());
    }
    Err(BrokerError::internal("private Broker did not become ready"))
}

fn stop_incompatible_broker(
    token: &str,
    protocol: u32,
    version: &str,
    pid: u32,
) -> Result<(), BrokerError> {
    match send_request(
        &Request::StopIfIdle {
            token: token.into(),
        },
        1_000,
    ) {
        Ok(Response::Stopping) => {}
        Ok(Response::Error { code, error, .. })
            if code == "invalid_argument"
                && error.contains("unknown variant")
                && error.contains("stop_if_idle") =>
        {
            return stop_legacy_idle_broker(pid, protocol, version);
        }
        Ok(Response::Error {
            code,
            error,
            retryable,
            command_id,
        }) => {
            return Err(BrokerError {
                code,
                error: format!(
                    "private Broker {version} (protocol {protocol}) cannot upgrade now: {error}"
                ),
                retryable,
                command_id,
            });
        }
        Ok(_) | Err(_) => {
            return Err(BrokerError::internal(format!(
                "private Broker {version} (protocol {protocol}) does not support safe in-place upgrade; stop it after active commands finish"
            )));
        }
    }
    let deadline = now_ms().saturating_add(START_WAIT_MS);
    while now_ms() < deadline {
        if UnixStream::connect(socket_path()).is_err() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(BrokerError::internal(
        "incompatible private Broker did not stop",
    ))
}

fn stop_legacy_idle_broker(pid: u32, protocol: u32, version: &str) -> Result<(), BrokerError> {
    let pid = i32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 1)
        .ok_or_else(|| {
            BrokerError::internal(format!("private Broker {version} returned an invalid PID"))
        })?;
    let isolated_socket = runtime_home().join(format!(
        ".broker-upgrade-{pid}-{}.sock",
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::rename(socket_path(), &isolated_socket).map_err(|error| {
        BrokerError::internal(format!(
            "failed to isolate private Broker {version} for upgrade: {error}"
        ))
    })?;

    // Existing accepted requests persist their record before dispatch. Once the
    // public socket is isolated, no new request can enter the legacy Broker.
    thread::sleep(Duration::from_millis(100));
    let active_count = match persisted_active_count() {
        Ok(count) => count,
        Err(error) => {
            restore_isolated_socket(&isolated_socket).map_err(BrokerError::internal)?;
            return Err(BrokerError::internal(error));
        }
    };
    if active_count > 0 {
        restore_isolated_socket(&isolated_socket).map_err(BrokerError::internal)?;
        return Err(BrokerError {
            code: "target_busy".into(),
            error: format!(
                "private Broker {version} (protocol {protocol}) cannot upgrade now: {active_count} persisted command(s) are still active"
            ),
            retryable: true,
            command_id: None,
        });
    }

    if unsafe { kill(pid, 15) } != 0 {
        let signal_error = std::io::Error::last_os_error();
        if signal_error.raw_os_error() == Some(3) {
            let _ = fs::remove_file(&isolated_socket);
            return Ok(());
        }
        restore_isolated_socket(&isolated_socket).map_err(BrokerError::internal)?;
        return Err(BrokerError::internal(format!(
            "failed to stop idle private Broker {version}: {}",
            signal_error
        )));
    }
    let deadline = now_ms().saturating_add(START_WAIT_MS);
    while now_ms() < deadline {
        if UnixStream::connect(&isolated_socket).is_err() {
            let _ = fs::remove_file(&isolated_socket);
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    restore_isolated_socket(&isolated_socket).map_err(BrokerError::internal)?;
    Err(BrokerError::internal(format!(
        "idle private Broker {version} did not stop"
    )))
}

fn restore_isolated_socket(isolated_socket: &Path) -> Result<(), String> {
    if socket_path().exists() {
        return Err("cannot restore legacy Broker socket because its path is occupied".into());
    }
    fs::rename(isolated_socket, socket_path())
        .map_err(|error| format!("failed to restore legacy Broker socket: {error}"))
}

fn persisted_active_count() -> Result<usize, String> {
    let mut active_count = 0;
    for entry in fs::read_dir(commands_dir())
        .map_err(|error| format!("failed to inspect legacy Broker commands: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to inspect Broker command: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("failed to inspect Broker command record: {error}"))?;
        if !metadata.file_type().is_file() {
            return Err("legacy Broker command record must be a regular file".into());
        }
        if metadata.len()
            > (MAX_OUTPUT_BYTES as u64)
                .saturating_mul(2)
                .saturating_add(1_048_576)
        {
            return Err("legacy Broker command record is too large to validate safely".into());
        }
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(&path)
                .map_err(|error| format!("failed to read Broker command record: {error}"))?,
        )
        .map_err(|error| format!("failed to validate Broker command record: {error}"))?;
        let status = value
            .pointer("/descriptor/status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "legacy Broker command record has no status".to_string())?;
        if !is_terminal(status) {
            active_count += 1;
        }
    }
    Ok(active_count)
}

fn unwrap_response(response: Response) -> Result<Response, BrokerError> {
    match response {
        Response::Error {
            code,
            error,
            retryable,
            command_id,
        } => Err(BrokerError {
            code,
            error,
            retryable,
            command_id,
        }),
        response => Ok(response),
    }
}

pub fn is_child() -> bool {
    std::env::var_os(CHILD_MARKER).as_deref() == Some(std::ffi::OsStr::new("1"))
}

/// Publish a ref-producing AX snapshot to the private Broker side channel.
pub fn publish_observation(pid: i32, snapshot: &crate::ax::SnapshotResult) {
    let Some(path) = std::env::var_os(OBSERVATION_FILE).map(PathBuf::from) else {
        return;
    };
    let window_id = crate::ax::focused_window_geom(pid)
        .map(|window| window.window_id)
        .unwrap_or(0);
    if window_id == 0 {
        return;
    }
    let value = serde_json::json!({
        "pid": pid,
        "bundle_id": crate::system::bundle_id_for_pid(pid).unwrap_or_default(),
        "window_id": window_id,
        "limit": snapshot.limit,
        "elements": snapshot.elements,
    });
    let Ok(encoded) = serde_json::to_vec(&value) else {
        return;
    };
    let mut file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return,
    };
    let _ = file.write_all(&encoded);
}

pub fn run(options: RunOptions) -> Result<RunResult, BrokerError> {
    let token = ensure_running()?;
    let executable = std::env::current_exe().map_err(|error| {
        BrokerError::internal(format!("failed to locate cu executable: {error}"))
    })?;
    let response = send_request(
        &Request::Run {
            token,
            executable,
            client_key: options.client_key,
            request_id: options.request_id,
            timeout_ms: options.timeout_ms,
            command: options.command,
            argv: options.argv,
            mutating: options.mutating,
            resource: options.resource,
            desktop_lock: options.desktop_lock,
            ref_id: options.ref_id,
            observation_id: options.observation_id,
            output_dir: options.output_dir,
            test_frontmost_override: options.test_frontmost_override,
        },
        options.timeout_ms,
    )
    .map_err(BrokerError::internal)?;
    match unwrap_response(response)? {
        Response::Run { result } => Ok(result),
        _ => Err(BrokerError::internal(
            "private Broker returned the wrong response type",
        )),
    }
}

pub fn status(client_key: String) -> Result<BrokerStatus, BrokerError> {
    let token = ensure_running()?;
    match unwrap_response(
        send_request(&Request::Status { token, client_key }, 5_000)
            .map_err(BrokerError::internal)?,
    )? {
        Response::Status { status } => Ok(status),
        _ => Err(BrokerError::internal(
            "private Broker returned the wrong response type",
        )),
    }
}

pub fn commands(
    client_key: String,
    limit: usize,
    statuses: Vec<String>,
) -> Result<Vec<CommandRecord>, BrokerError> {
    let token = ensure_running()?;
    match unwrap_response(
        send_request(
            &Request::Commands {
                token,
                client_key,
                limit,
                statuses,
            },
            5_000,
        )
        .map_err(BrokerError::internal)?,
    )? {
        Response::Commands { commands } => Ok(commands),
        _ => Err(BrokerError::internal(
            "private Broker returned the wrong response type",
        )),
    }
}

pub fn command(client_key: String, command_id: String) -> Result<CommandRecord, BrokerError> {
    let token = ensure_running()?;
    match unwrap_response(
        send_request(
            &Request::Command {
                token,
                client_key,
                command_id,
            },
            5_000,
        )
        .map_err(BrokerError::internal)?,
    )? {
        Response::Command { command } => Ok(command),
        _ => Err(BrokerError::internal(
            "private Broker returned the wrong response type",
        )),
    }
}

pub fn cancel(client_key: String, command_id: String) -> Result<CommandRecord, BrokerError> {
    let token = ensure_running()?;
    match unwrap_response(
        send_request(
            &Request::Cancel {
                token,
                client_key,
                command_id,
            },
            5_000,
        )
        .map_err(BrokerError::internal)?,
    )? {
        Response::Command { command } => Ok(command),
        _ => Err(BrokerError::internal(
            "private Broker returned the wrong response type",
        )),
    }
}

pub fn serve() -> Result<(), String> {
    prepare_home()?;
    let token = read_token()?;
    let path = socket_path();
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .map_err(|error| format!("failed to bind private Broker socket: {error}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to secure private Broker socket: {error}"))?;
    let _ = fs::remove_file(start_lock_path());
    let shared = Arc::new((Mutex::new(load_state()?), Condvar::new()));
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&shared);
                let expected_token = token.clone();
                thread::spawn(move || handle_connection(stream, state, expected_token));
            }
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: UnixStream, shared: SharedState, expected_token: String) {
    let cloned = match stream.try_clone() {
        Ok(cloned) => cloned,
        Err(_) => return,
    };
    let mut line = String::new();
    if BufReader::new(cloned).read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let request: Request = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_response(
                &mut stream,
                &error_response(
                    "invalid_argument",
                    format!("invalid private request: {error}"),
                    false,
                ),
            );
            return;
        }
    };
    if request_token(&request) != expected_token {
        let _ = write_response(
            &mut stream,
            &error_response(
                "permission_denied",
                "private Broker authentication failed",
                false,
            ),
        );
        return;
    }
    let response = route(request, shared);
    let stopping = matches!(response, Response::Stopping);
    let _ = write_response(&mut stream, &response);
    if stopping {
        drop(stream);
        let _ = fs::remove_file(socket_path());
        std::process::exit(0);
    }
}

fn request_token(request: &Request) -> &str {
    match request {
        Request::Ping { token }
        | Request::Run { token, .. }
        | Request::Status { token, .. }
        | Request::Commands { token, .. }
        | Request::Command { token, .. }
        | Request::Cancel { token, .. }
        | Request::StopIfIdle { token } => token,
    }
}

fn write_response(stream: &mut UnixStream, response: &Response) -> Result<(), String> {
    let mut payload = serde_json::to_vec(response)
        .map_err(|error| format!("failed to encode private response: {error}"))?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .map_err(|error| format!("failed to write private response: {error}"))
}

fn route(request: Request, shared: SharedState) -> Response {
    match request {
        Request::Ping { .. } => Response::Pong {
            protocol: INTERNAL_PROTOCOL,
            pid: std::process::id(),
            version: crate::VERSION.into(),
        },
        Request::Run {
            executable,
            client_key,
            request_id,
            timeout_ms,
            command,
            argv,
            mutating,
            resource,
            desktop_lock,
            ref_id,
            observation_id,
            output_dir,
            test_frontmost_override,
            ..
        } => run_command(
            shared,
            RunOptions {
                client_key,
                request_id,
                timeout_ms,
                command,
                argv,
                mutating,
                resource,
                desktop_lock,
                ref_id,
                observation_id,
                output_dir,
                test_frontmost_override,
            },
            executable,
        ),
        Request::Status { client_key, .. } => broker_status(&shared, &client_key),
        Request::Commands {
            client_key,
            limit,
            statuses,
            ..
        } => list_commands(&shared, &client_key, limit, &statuses),
        Request::Command {
            client_key,
            command_id,
            ..
        } => get_command(&shared, &client_key, &command_id),
        Request::Cancel {
            client_key,
            command_id,
            ..
        } => cancel_command(&shared, &client_key, &command_id),
        Request::StopIfIdle { .. } => stop_if_idle(&shared),
    }
}

fn validate_run(options: &RunOptions) -> Result<(), Box<Response>> {
    if options.timeout_ms == 0 || options.timeout_ms > MAX_TIMEOUT_MS {
        return Err(Box::new(error_response(
            "invalid_argument",
            format!("--timeout must be from 1 through {MAX_TIMEOUT_MS} milliseconds"),
            false,
        )));
    }
    if options.argv.is_empty() {
        return Err(Box::new(error_response(
            "invalid_argument",
            "private command argv is empty",
            false,
        )));
    }
    Ok(())
}

fn run_command(shared: SharedState, options: RunOptions, executable: PathBuf) -> Response {
    if let Err(error) = validate_run(&options) {
        return *error;
    }
    let fingerprint = format!(
        "{}\0{}\0{}\0{}",
        options.command,
        options.argv.join("\0"),
        options.output_dir.as_deref().unwrap_or(""),
        options.test_frontmost_override.as_deref().unwrap_or("")
    );
    let accepted_at_ms = now_ms();
    let deadline_at_ms = accepted_at_ms.saturating_add(options.timeout_ms);
    let command_id = format!(
        "command:{}-{}-{}",
        std::process::id(),
        accepted_at_ms,
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );

    let expectation = {
        let (mutex, _) = &*shared;
        let mut state = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.draining {
            return error_response(
                "target_busy",
                "private Broker is draining for an upgrade; retry shortly",
                true,
            );
        }
        if let Some(request_id) = &options.request_id
            && let Some(existing_id) = state
                .requests
                .get(&(options.client_key.clone(), request_id.clone()))
                .cloned()
            && let Some(existing) = state.records.get(&existing_id)
        {
            if existing.fingerprint != fingerprint {
                return Response::Error {
                    code: "request_id_conflict".into(),
                    error: "request ID was reused for a different command".into(),
                    retryable: false,
                    command_id: Some(existing_id),
                };
            }
            if is_terminal(&existing.descriptor.status) {
                return Response::Run {
                    result: RunResult {
                        command: existing.descriptor.clone(),
                        stdout: existing.stdout.clone(),
                        stderr: existing.stderr.clone(),
                        exit_code: existing.descriptor.exit_code.unwrap_or(1),
                        replayed: true,
                    },
                };
            }
            return Response::Error {
                code: "command_in_progress".into(),
                error: "request ID already belongs to an active command".into(),
                retryable: true,
                command_id: Some(existing_id),
            };
        }
        sweep_records(&mut state);
        let expectation = match resolve_expectation(&state, &options) {
            Ok(expectation) => expectation,
            Err(error) => return *error,
        };
        let descriptor = CommandRecord {
            id: command_id.clone(),
            client_key: options.client_key.clone(),
            request_id: options.request_id.clone(),
            command: options.command.clone(),
            mutating: options.mutating,
            status: "accepted".into(),
            accepted_at_ms,
            deadline_at_ms,
            dispatched_at_ms: None,
            completed_at_ms: None,
            resource: options.resource.clone(),
            desktop_lock: options.desktop_lock,
            cancellation_requested: false,
            exit_code: None,
        };
        let stored = StoredRecord {
            descriptor,
            fingerprint,
            stdout: String::new(),
            stderr: String::new(),
        };
        if let Err(error) = persist_record(&stored) {
            return error_response(
                "internal_error",
                format!("failed to persist accepted command: {error}"),
                false,
            );
        }
        state.records.insert(command_id.clone(), stored);
        state.order.push(command_id.clone());
        if let Some(request_id) = &options.request_id {
            state.requests.insert(
                (options.client_key.clone(), request_id.clone()),
                command_id.clone(),
            );
        }
        expectation
    };

    if !acquire_resource(&shared, &command_id, &options) {
        return response_for_record(&shared, &command_id, false);
    }

    let response = execute_child(
        &shared,
        &command_id,
        &options,
        expectation.as_ref(),
        &executable,
    );
    release_resource(&shared, &options);
    response
}

fn resolve_expectation(
    state: &BrokerState,
    options: &RunOptions,
) -> Result<Option<ObservationExpectation>, Box<Response>> {
    let Some(ref_id) = options.ref_id else {
        return Ok(None);
    };
    let resource = options.resource.as_deref().ok_or_else(|| {
        Box::new(error_response(
            "observation_required",
            "ref actions require an explicit target application",
            false,
        ))
    })?;
    let observation_id = if let Some(observation_id) = &options.observation_id {
        observation_id.clone()
    } else {
        state
            .latest_observations
            .get(&(options.client_key.clone(), resource.to_string()))
            .cloned()
            .ok_or_else(|| {
                Box::new(error_response(
                    "observation_required",
                    "no current Observation exists for this client and target; run cu snapshot first",
                    false,
                ))
            })?
    };
    let observation = state.observations.get(&observation_id).ok_or_else(|| {
        Box::new(error_response(
            "observation_not_found",
            "Observation is missing or expired; run cu snapshot again",
            false,
        ))
    })?;
    if observation.client_key != options.client_key {
        return Err(Box::new(error_response(
            "observation_not_found",
            "Observation does not belong to this client key",
            false,
        )));
    }
    if observation.resource != resource {
        return Err(Box::new(error_response(
            "stale_observation",
            "Observation belongs to a different application target",
            false,
        )));
    }
    if now_ms().saturating_sub(observation.created_at_ms) > OBSERVATION_TTL_MS {
        return Err(Box::new(error_response(
            "observation_not_found",
            "Observation expired; run cu snapshot again",
            false,
        )));
    }
    let target = observation
        .elements
        .iter()
        .find(|element| element.ref_id == ref_id)
        .cloned()
        .ok_or_else(|| {
            Box::new(error_response(
                "stale_observation",
                format!("ref [{ref_id}] was not present in the selected Observation"),
                false,
            ))
        })?;
    Ok(Some(ObservationExpectation {
        observation_id: observation.id.clone(),
        client_key: observation.client_key.clone(),
        pid: observation.pid,
        bundle_id: observation.bundle_id.clone(),
        window_id: observation.window_id,
        ax_generation: observation.ax_generation,
        limit: observation.limit,
        target,
        elements: observation.elements.clone(),
    }))
}

fn acquire_resource(shared: &SharedState, command_id: &str, options: &RunOptions) -> bool {
    let (mutex, changed) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        let Some(record) = state.records.get(command_id) else {
            return false;
        };
        if record.descriptor.cancellation_requested {
            finish_without_dispatch(
                &mut state,
                command_id,
                "cancelled",
                "command_cancelled",
                "command was cancelled before dispatch",
            );
            return false;
        }
        if now_ms() >= record.descriptor.deadline_at_ms {
            finish_without_dispatch(
                &mut state,
                command_id,
                "expired",
                "command_expired",
                "command deadline elapsed before dispatch",
            );
            return false;
        }
        if resource_available(&state, options) {
            if let Some(resource) = &options.resource {
                if options.mutating {
                    state.writers.insert(resource.clone());
                } else {
                    *state.readers.entry(resource.clone()).or_insert(0) += 1;
                }
            }
            if options.mutating {
                state.active_mutations += 1;
            }
            if options.desktop_lock {
                state.desktop_active = true;
            }
            return true;
        }
        let remaining = record.descriptor.deadline_at_ms.saturating_sub(now_ms());
        let wait = Duration::from_millis(remaining.min(100));
        let (next, _) = changed
            .wait_timeout(state, wait)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next;
    }
}

fn resource_available(state: &BrokerState, options: &RunOptions) -> bool {
    if options.desktop_lock {
        return !state.desktop_active && state.active_mutations == 0;
    }
    if options.mutating && state.desktop_active {
        return false;
    }
    let Some(resource) = &options.resource else {
        return !options.mutating || !state.desktop_active;
    };
    if options.mutating {
        !state.writers.contains(resource) && state.readers.get(resource).copied().unwrap_or(0) == 0
    } else {
        !state.writers.contains(resource)
    }
}

fn release_resource(shared: &SharedState, options: &RunOptions) {
    let (mutex, changed) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(resource) = &options.resource {
        if options.mutating {
            state.writers.remove(resource);
        } else if let Some(count) = state.readers.get_mut(resource) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.readers.remove(resource);
            }
        }
    }
    if options.mutating {
        state.active_mutations = state.active_mutations.saturating_sub(1);
    }
    if options.desktop_lock {
        state.desktop_active = false;
    }
    changed.notify_all();
}

fn execute_child(
    shared: &SharedState,
    command_id: &str,
    options: &RunOptions,
    expectation: Option<&ObservationExpectation>,
    executable: &Path,
) -> Response {
    let mut command = Command::new(executable);
    let observation_path = runtime_home().join(format!("{command_id}.observation.json"));
    command
        .args(&options.argv)
        .env(CHILD_MARKER, "1")
        .env(INTERNAL_CLIENT_KEY, &options.client_key)
        .env(OBSERVATION_FILE, &observation_path)
        .env_remove(OUTPUT_DIR)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            if setpgid(0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    if let Some(output_dir) = &options.output_dir {
        command.env(OUTPUT_DIR, output_dir);
    }
    if let Some(frontmost) = &options.test_frontmost_override {
        command.env(TEST_FRONTMOST_OVERRIDE, frontmost);
    }
    if let Some(expectation) = expectation {
        let encoded = serde_json::to_string(expectation).unwrap_or_default();
        command.env(EXPECTED_OBSERVATION, encoded);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            finish_spawn_error(
                shared,
                command_id,
                format!("failed to dispatch cu: {error}"),
            );
            return response_for_record(shared, command_id, false);
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    {
        let (mutex, _) = &**shared;
        let mut state = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.child_pids.insert(command_id.into(), child.id());
        let updated = if let Some(record) = state.records.get_mut(command_id) {
            record.descriptor.status = "dispatched".into();
            record.descriptor.dispatched_at_ms = Some(now_ms());
            Some(record.clone())
        } else {
            None
        };
        if let Some(record) = updated {
            let _ = persist_record(&record);
        }
    }

    let mut forced_status = None;
    let mut completed_exit_code = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                completed_exit_code = Some(status.code().unwrap_or(1));
                break;
            }
            Ok(None) => {}
            Err(error) => {
                forced_status = Some((
                    "unknown_outcome",
                    "unknown_outcome",
                    format!("lost command outcome after dispatch: {error}"),
                ));
                terminate_child_group(&mut child);
                break;
            }
        }
        let (cancelled, deadline_at_ms) = {
            let (mutex, _) = &**shared;
            let state = mutex
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state
                .records
                .get(command_id)
                .map(|record| {
                    (
                        record.descriptor.cancellation_requested,
                        record.descriptor.deadline_at_ms,
                    )
                })
                .unwrap_or((true, 0))
        };
        if cancelled {
            forced_status = Some(if options.mutating {
                (
                    "unknown_outcome",
                    "unknown_outcome",
                    "mutation was cancelled after dispatch; inspect current UI state".into(),
                )
            } else {
                (
                    "cancelled",
                    "command_cancelled",
                    "command was cancelled".into(),
                )
            });
            terminate_child_group(&mut child);
            break;
        }
        if now_ms() >= deadline_at_ms {
            forced_status = Some(if options.mutating {
                (
                    "unknown_outcome",
                    "unknown_outcome",
                    "command deadline elapsed after mutation dispatch; inspect current UI state"
                        .into(),
                )
            } else {
                (
                    "expired",
                    "command_expired",
                    "command deadline elapsed".into(),
                )
            });
            terminate_child_group(&mut child);
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    let mut stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    if let Some((status, code, error)) = forced_status {
        let _ = fs::remove_file(&observation_path);
        let _ = child.wait();
        let mut stderr = stderr;
        if !stderr.is_empty() {
            stderr.push('\n');
        }
        stderr.push_str(
            &serde_json::json!({
                "schema_version": crate::MACHINE_SCHEMA_VERSION,
                "ok": false,
                "code": code,
                "error": error,
                "retryable": false,
                "command_id": command_id,
            })
            .to_string(),
        );
        finish_child(shared, command_id, status, stdout, stderr, 1);
    } else {
        if completed_exit_code == Some(0) {
            stdout = capture_observation(shared, options, &observation_path, stdout);
        } else {
            let _ = fs::remove_file(&observation_path);
        }
        finish_child(
            shared,
            command_id,
            "completed",
            stdout,
            stderr,
            completed_exit_code.unwrap_or(1),
        );
    }
    response_for_record(shared, command_id, false)
}

fn terminate_child_group(child: &mut std::process::Child) {
    let pid = child.id();
    if pid > 0 && pid <= i32::MAX as u32 {
        let result = unsafe { kill(-(pid as i32), 9) };
        if result == 0 {
            return;
        }
    }
    let _ = child.kill();
}

fn read_bounded<R: Read>(reader: Option<R>) -> String {
    let Some(reader) = reader else {
        return String::new();
    };
    let mut bytes = Vec::new();
    let _ = reader.take(MAX_OUTPUT_BYTES as u64).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn capture_observation(
    shared: &SharedState,
    options: &RunOptions,
    observation_path: &Path,
    stdout: String,
) -> String {
    let Ok(mut output) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        let source = fs::read(observation_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let _ = fs::remove_file(observation_path);
        if let Some(source) = source {
            store_observation(shared, options, &source);
        }
        return stdout;
    };
    let source = fs::read(observation_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .or_else(|| {
            output
                .as_object_mut()
                .and_then(|object| object.remove("_computer_pilot_observation"))
        })
        .unwrap_or_else(|| output.clone());
    let _ = fs::remove_file(observation_path);
    let Some((record, observation_id)) = store_observation(shared, options, &source) else {
        return serde_json::to_string(&output).unwrap_or(stdout);
    };
    if let Some(object) = output.as_object_mut() {
        object.insert("observation_id".into(), observation_id.into());
        object.insert("client_key".into(), options.client_key.clone().into());
        object.insert("pid".into(), i64::from(record.pid).into());
        object.insert("bundle_id".into(), record.bundle_id.into());
        object.insert("window_id".into(), u64::from(record.window_id).into());
        object.insert("ax_generation".into(), record.ax_generation.into());
    }
    serde_json::to_string(&output).unwrap_or(stdout)
}

fn store_observation(
    shared: &SharedState,
    options: &RunOptions,
    source: &serde_json::Value,
) -> Option<(ObservationRecord, String)> {
    let source = source.as_object()?;
    let pid = source.get("pid").and_then(serde_json::Value::as_i64)?;
    let window_id = source
        .get("window_id")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    let elements_value = source.get("elements")?;
    let elements = serde_json::from_value::<Vec<ObservedElement>>(elements_value.clone()).ok()?;
    let resource = options
        .resource
        .clone()
        .unwrap_or_else(|| format!("pid:{pid}"));
    let bundle_id = source
        .get("bundle_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let limit = source
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(elements.len());
    let ax_generation = observed_generation(&elements);
    let observation_id = format!(
        "observation:{}-{}-{}",
        std::process::id(),
        now_ms(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let record = ObservationRecord {
        id: observation_id.clone(),
        client_key: options.client_key.clone(),
        resource: resource.clone(),
        pid: pid as i32,
        bundle_id: bundle_id.clone(),
        window_id,
        ax_generation,
        limit,
        elements,
        created_at_ms: now_ms(),
    };
    {
        let (mutex, _) = &**shared;
        let mut state = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.latest_observations.insert(
            (options.client_key.clone(), resource),
            observation_id.clone(),
        );
        state
            .observations
            .insert(observation_id.clone(), record.clone());
        sweep_observations(&mut state);
    }
    Some((record, observation_id))
}

fn observed_generation(elements: &[ObservedElement]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for element in elements {
        element.ref_id.hash(&mut hasher);
        element.role.hash(&mut hasher);
        element.title.hash(&mut hasher);
        element.value.hash(&mut hasher);
        element.x.to_bits().hash(&mut hasher);
        element.y.to_bits().hash(&mut hasher);
        element.width.to_bits().hash(&mut hasher);
        element.height.to_bits().hash(&mut hasher);
        element.ax_path.hash(&mut hasher);
    }
    hasher.finish()
}

fn current_elements(elements: &[crate::ax::Element]) -> Vec<ObservedElement> {
    elements
        .iter()
        .map(|element| ObservedElement {
            ref_id: element.ref_id,
            role: element.role.clone(),
            title: element.title.clone(),
            value: element.value.clone(),
            x: element.x,
            y: element.y,
            width: element.width,
            height: element.height,
            ax_path: element.ax_path.clone(),
        })
        .collect()
}

fn same_element(left: &ObservedElement, right: &ObservedElement) -> bool {
    left.ref_id == right.ref_id
        && left.role == right.role
        && left.title == right.title
        && left.value == right.value
        && left.x.to_bits() == right.x.to_bits()
        && left.y.to_bits() == right.y.to_bits()
        && left.width.to_bits() == right.width.to_bits()
        && left.height.to_bits() == right.height.to_bits()
        && left.ax_path == right.ax_path
}

/// Validate the Broker-selected Observation immediately before a ref action.
pub fn enforce_expected_observation(
    pid: i32,
    app_name: &str,
    ref_id: usize,
) -> Result<(), CuError> {
    let Some(encoded) = std::env::var_os(EXPECTED_OBSERVATION) else {
        return Ok(());
    };
    let expected: ObservationExpectation = serde_json::from_str(&encoded.to_string_lossy())
        .map_err(|_| CuError::msg("invalid private Observation expectation"))?;
    let stale = |reason: String| {
        CuError::msg(reason)
            .with_code(ErrorCode::StaleObservation)
            .with_hint("run cu snapshot again and use the new observation_id/ref")
    };
    if expected.target.ref_id != ref_id {
        return Err(stale(format!(
            "Observation {} does not contain requested ref [{ref_id}]",
            expected.observation_id
        )));
    }
    if expected.pid != pid {
        return Err(stale(format!(
            "target PID changed from {} to {pid}",
            expected.pid
        )));
    }
    let bundle_id = crate::system::bundle_id_for_pid(pid).unwrap_or_default();
    if expected.bundle_id != bundle_id {
        return Err(stale("target application identity changed".into()));
    }
    let window_id = crate::ax::focused_window_geom(pid)
        .map(|window| window.window_id)
        .unwrap_or(0);
    if expected.window_id != window_id {
        return Err(stale(format!(
            "target window changed from {} to {window_id}",
            expected.window_id
        )));
    }
    let snapshot = crate::ax::snapshot(pid, app_name, expected.limit);
    if !snapshot.ok {
        return Err(stale(
            snapshot
                .error
                .unwrap_or_else(|| "could not refresh target UI".into()),
        ));
    }
    let elements = current_elements(&snapshot.elements);
    if observed_generation(&elements) != expected.ax_generation {
        return Err(stale(
            "UI changed after the selected Observation; the ref was not dispatched".into(),
        ));
    }
    let current = elements
        .iter()
        .find(|element| element.ref_id == ref_id)
        .ok_or_else(|| stale(format!("ref [{ref_id}] no longer exists")))?;
    if !same_element(current, &expected.target) {
        return Err(stale(format!(
            "ref [{ref_id}] now identifies a different element"
        )));
    }
    Ok(())
}

fn finish_child(
    shared: &SharedState,
    command_id: &str,
    status: &str,
    stdout: String,
    stderr: String,
    exit_code: i32,
) {
    let (mutex, changed) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.child_pids.remove(command_id);
    let updated = if let Some(record) = state.records.get_mut(command_id) {
        record.descriptor.status = status.into();
        record.descriptor.completed_at_ms = Some(now_ms());
        record.descriptor.exit_code = Some(exit_code);
        record.stdout = stdout;
        record.stderr = stderr;
        Some(record.clone())
    } else {
        None
    };
    if let Some(record) = updated {
        let _ = persist_record(&record);
    }
    changed.notify_all();
}

fn finish_spawn_error(shared: &SharedState, command_id: &str, error: String) {
    let stderr = serde_json::json!({
        "schema_version": crate::MACHINE_SCHEMA_VERSION,
        "ok": false,
        "code": "internal_error",
        "error": error,
        "retryable": false,
        "command_id": command_id,
    })
    .to_string();
    finish_child(shared, command_id, "completed", String::new(), stderr, 1);
}

fn finish_without_dispatch(
    state: &mut BrokerState,
    command_id: &str,
    status: &str,
    code: &str,
    error: &str,
) {
    let updated = if let Some(record) = state.records.get_mut(command_id) {
        record.descriptor.status = status.into();
        record.descriptor.completed_at_ms = Some(now_ms());
        record.descriptor.exit_code = Some(1);
        record.stderr = serde_json::json!({
            "schema_version": crate::MACHINE_SCHEMA_VERSION,
            "ok": false,
            "code": code,
            "error": error,
            "retryable": false,
            "command_id": command_id,
        })
        .to_string();
        Some(record.clone())
    } else {
        None
    };
    if let Some(record) = updated {
        let _ = persist_record(&record);
    }
}

fn response_for_record(shared: &SharedState, command_id: &str, replayed: bool) -> Response {
    let (mutex, _) = &**shared;
    let state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(record) = state.records.get(command_id) else {
        return error_response("command_not_found", "command record disappeared", false);
    };
    Response::Run {
        result: RunResult {
            command: record.descriptor.clone(),
            stdout: record.stdout.clone(),
            stderr: record.stderr.clone(),
            exit_code: record.descriptor.exit_code.unwrap_or(1),
            replayed,
        },
    }
}

fn broker_status(shared: &SharedState, client_key: &str) -> Response {
    let (mutex, _) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    sweep_records(&mut state);
    let records: Vec<_> = state
        .records
        .values()
        .filter(|record| record.descriptor.client_key == client_key)
        .collect();
    Response::Status {
        status: BrokerStatus {
            running: true,
            pid: std::process::id(),
            protocol: INTERNAL_PROTOCOL,
            command_count: records.len(),
            active_count: records
                .iter()
                .filter(|record| !is_terminal(&record.descriptor.status))
                .count(),
            uncertain_count: records
                .iter()
                .filter(|record| record.descriptor.status == "unknown_outcome")
                .count(),
        },
    }
}

fn list_commands(
    shared: &SharedState,
    client_key: &str,
    limit: usize,
    statuses: &[String],
) -> Response {
    let (mutex, _) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    sweep_records(&mut state);
    let filter: HashSet<&str> = statuses.iter().map(String::as_str).collect();
    let commands = state
        .order
        .iter()
        .rev()
        .filter_map(|id| state.records.get(id))
        .filter(|record| record.descriptor.client_key == client_key)
        .filter(|record| filter.is_empty() || filter.contains(record.descriptor.status.as_str()))
        .take(limit.min(100))
        .map(|record| record.descriptor.clone())
        .collect();
    Response::Commands { commands }
}

fn get_command(shared: &SharedState, client_key: &str, command_id: &str) -> Response {
    let (mutex, _) = &**shared;
    let state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match state.records.get(command_id) {
        Some(record) if record.descriptor.client_key == client_key => Response::Command {
            command: record.descriptor.clone(),
        },
        _ => error_response(
            "command_not_found",
            "command was not found for this client key",
            false,
        ),
    }
}

fn cancel_command(shared: &SharedState, client_key: &str, command_id: &str) -> Response {
    let (mutex, changed) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (descriptor, updated) = match state.records.get_mut(command_id) {
        Some(record) if record.descriptor.client_key == client_key => {
            if !is_terminal(&record.descriptor.status) {
                record.descriptor.cancellation_requested = true;
            }
            (record.descriptor.clone(), record.clone())
        }
        _ => {
            return error_response(
                "command_not_found",
                "command was not found for this client key",
                false,
            );
        }
    };
    if let Err(error) = persist_record(&updated) {
        return error_response(
            "internal_error",
            format!("failed to persist command cancellation: {error}"),
            false,
        );
    }
    changed.notify_all();
    Response::Command {
        command: descriptor,
    }
}

fn sweep_records(state: &mut BrokerState) {
    sweep_observations(state);
    while state.order.len() > MAX_RECORDS {
        let id = state.order.remove(0);
        let removable = state
            .records
            .get(&id)
            .map(|record| is_terminal(&record.descriptor.status))
            .unwrap_or(true);
        if removable {
            if let Some(record) = state.records.remove(&id)
                && let Some(request_id) = record.descriptor.request_id
            {
                remove_persisted_record(&id);
                state
                    .requests
                    .remove(&(record.descriptor.client_key, request_id));
            } else {
                remove_persisted_record(&id);
            }
        } else {
            state.order.push(id);
            break;
        }
    }
}

fn stop_if_idle(shared: &SharedState) -> Response {
    let (mutex, _) = &**shared;
    let mut state = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let active_count = state
        .records
        .values()
        .filter(|record| !is_terminal(&record.descriptor.status))
        .count();
    if active_count > 0 {
        return error_response(
            "target_busy",
            format!("{active_count} command(s) are still active; retry after they finish"),
            true,
        );
    }
    state.draining = true;
    Response::Stopping
}

fn sweep_observations(state: &mut BrokerState) {
    let now = now_ms();
    state.observations.retain(|_, observation| {
        now.saturating_sub(observation.created_at_ms) <= OBSERVATION_TTL_MS
    });
    while state.observations.len() > MAX_OBSERVATIONS {
        let Some(oldest_id) = state
            .observations
            .iter()
            .min_by_key(|(_, observation)| observation.created_at_ms)
            .map(|(id, _)| id.clone())
        else {
            break;
        };
        state.observations.remove(&oldest_id);
    }
    state
        .latest_observations
        .retain(|_, observation_id| state.observations.contains_key(observation_id));
}

fn is_terminal(status: &str) -> bool {
    matches!(
        status,
        "completed" | "cancelled" | "expired" | "unknown_outcome"
    )
}

pub fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

pub fn validate_client_key(value: &str) -> Result<(), String> {
    let valid = (3..=128).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b':' | b'-' => index > 0,
            _ => false,
        });
    if valid {
        Ok(())
    } else {
        Err("client key must be 3-128 characters using letters, digits, dot, underscore, colon, or hyphen".into())
    }
}

pub fn validate_request_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
        Err("request ID must be 1-128 non-whitespace characters".into())
    } else {
        Ok(())
    }
}

#[allow(dead_code)]
fn path_is_private(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
        .unwrap_or(false)
}
