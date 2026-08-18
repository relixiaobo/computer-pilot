//! Structured errors. Lets failure paths attach hints, suggested next commands,
//! and arbitrary diagnostics that the agent can use to recover without
//! re-snapshotting.
//!
//! Construction is fluent:
//! ```ignore
//! Err(CuError::msg("element [5] is disabled")
//!     .with_hint("AXEnabled=false; try `cu wait --ref 5 --enabled` first")
//!     .with_next("cu perform 5 AXShowMenu"))
//! ```
//!
//! `From<String>` and `From<&str>` are implemented so plain string errors
//! propagate via `?` without ceremony — only failure sites that have something
//! useful to add need to use the builder.

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    CommandFailed,
    InvalidArgument,
    ObservationRequired,
    ObservationNotFound,
    StaleObservation,
    RequestIdConflict,
    CommandInProgress,
    CommandNotFound,
    CommandCancelled,
    CommandExpired,
    UnknownOutcome,
    TargetBusy,
    AmbiguousTarget,
    PermissionDenied,
    AppNotFound,
    WindowNotFound,
    FocusFailed,
    CaptureProtected,
    VerificationFailed,
    InternalError,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::CommandFailed => "command_failed",
            Self::InvalidArgument => "invalid_argument",
            Self::ObservationRequired => "observation_required",
            Self::ObservationNotFound => "observation_not_found",
            Self::StaleObservation => "stale_observation",
            Self::RequestIdConflict => "request_id_conflict",
            Self::CommandInProgress => "command_in_progress",
            Self::CommandNotFound => "command_not_found",
            Self::CommandCancelled => "command_cancelled",
            Self::CommandExpired => "command_expired",
            Self::UnknownOutcome => "unknown_outcome",
            Self::TargetBusy => "target_busy",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::PermissionDenied => "permission_denied",
            Self::AppNotFound => "app_not_found",
            Self::WindowNotFound => "window_not_found",
            Self::FocusFailed => "focus_failed",
            Self::CaptureProtected => "capture_protected",
            Self::VerificationFailed => "verification_failed",
            Self::InternalError => "internal_error",
        }
    }

    fn classify(error: &str) -> Self {
        let error = error.to_ascii_lowercase();
        if error.contains("ambiguous target") {
            Self::AmbiguousTarget
        } else if error.contains("invalid pid selector") {
            Self::InvalidArgument
        } else if error.contains("capture-protected") || error.contains("capture protected") {
            Self::CaptureProtected
        } else if error.contains("permission denied")
            || error.contains("permission is required")
            || error.contains("not authorized")
            || error.contains("not authorised")
            || error.contains("not permitted")
            || error.contains("(-1743)")
        {
            Self::PermissionDenied
        } else if error.contains("app not running")
            || error.contains("app not found")
            || error.contains("application not running")
            || error.contains("application isn't running")
        {
            Self::AppNotFound
        } else if error.contains("window not found")
            || error.contains("no on-screen window")
            || error.contains("no window found")
            || error.contains("waiting for window")
        {
            Self::WindowNotFound
        } else if error.contains("focus failed") || error.contains("failed to activate") {
            Self::FocusFailed
        } else if error.contains("verification failed") || error.contains("could not verify") {
            Self::VerificationFailed
        } else if error.contains("timed out") || error.contains("timeout expired") {
            Self::CommandExpired
        } else {
            Self::CommandFailed
        }
    }
}

#[derive(Debug)]
pub struct CuError {
    pub code: ErrorCode,
    pub error: String,
    pub retryable: bool,
    pub hint: Option<String>,
    pub suggested_next: Vec<String>,
    pub diagnostics: Option<Value>,
}

impl CuError {
    pub fn msg(error: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::CommandFailed,
            error: error.into(),
            retryable: false,
            hint: None,
            suggested_next: Vec::new(),
            diagnostics: None,
        }
    }

    pub fn with_code(mut self, code: ErrorCode) -> Self {
        self.code = code;
        self
    }

    #[allow(dead_code)]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_next(mut self, next: impl Into<String>) -> Self {
        self.suggested_next.push(next.into());
        self
    }

    #[allow(dead_code)]
    pub fn with_diagnostics(mut self, diag: Value) -> Self {
        self.diagnostics = Some(diag);
        self
    }

    /// Render as a `{"ok": false, ...}` JSON object containing only the fields
    /// that were populated.
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::json!({
            "schema_version": crate::MACHINE_SCHEMA_VERSION,
            "ok": false,
            "code": self.code.as_str(),
            "error": self.error,
            "retryable": self.retryable,
        });
        if let Some(h) = &self.hint {
            obj["hint"] = Value::String(h.clone());
        }
        if !self.suggested_next.is_empty() {
            obj["suggested_next"] = self.suggested_next.clone().into();
        }
        if let Some(d) = &self.diagnostics {
            obj["diagnostics"] = d.clone();
        }
        obj
    }
}

impl From<String> for CuError {
    fn from(s: String) -> Self {
        let code = ErrorCode::classify(&s);
        Self::msg(s).with_code(code)
    }
}

impl From<&str> for CuError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_classification_maps_recoverable_failure_families() {
        let cases = [
            ("ambiguous target: two apps", ErrorCode::AmbiguousTarget),
            ("invalid PID selector", ErrorCode::InvalidArgument),
            ("capture-protected window", ErrorCode::CaptureProtected),
            ("capture protected content", ErrorCode::CaptureProtected),
            ("permission denied", ErrorCode::PermissionDenied),
            (
                "not authorized to send Apple events",
                ErrorCode::PermissionDenied,
            ),
            ("automation failed (-1743)", ErrorCode::PermissionDenied),
            ("app not running", ErrorCode::AppNotFound),
            ("application isn't running", ErrorCode::AppNotFound),
            ("window not found", ErrorCode::WindowNotFound),
            ("waiting for window timed out", ErrorCode::WindowNotFound),
            ("focus failed", ErrorCode::FocusFailed),
            ("failed to activate target", ErrorCode::FocusFailed),
            ("verification failed", ErrorCode::VerificationFailed),
            ("could not verify the action", ErrorCode::VerificationFailed),
            ("command timed out", ErrorCode::CommandExpired),
            ("timeout expired", ErrorCode::CommandExpired),
            ("unclassified failure", ErrorCode::CommandFailed),
        ];

        for (message, expected) in cases {
            assert_eq!(ErrorCode::classify(message), expected, "message={message}");
            let error: CuError = message.into();
            assert_eq!(error.code, expected, "message={message}");
        }
    }

    #[test]
    fn json_envelope_omits_unset_recovery_fields() {
        let value = CuError::msg("plain failure").to_json();
        assert_eq!(value["schema_version"], crate::MACHINE_SCHEMA_VERSION);
        assert_eq!(value["ok"], false);
        assert_eq!(value["code"], "command_failed");
        assert_eq!(value["error"], "plain failure");
        assert_eq!(value["retryable"], false);
        assert!(value.get("hint").is_none());
        assert!(value.get("suggested_next").is_none());
        assert!(value.get("diagnostics").is_none());
    }

    #[test]
    fn json_envelope_preserves_all_explicit_recovery_fields() {
        let value = CuError::msg("stale")
            .with_code(ErrorCode::StaleObservation)
            .retryable(true)
            .with_hint("snapshot again")
            .with_next("cu snapshot Finder")
            .with_next("cu click 3 --app Finder")
            .with_diagnostics(serde_json::json!({"ref": 3}))
            .to_json();

        assert_eq!(value["code"], "stale_observation");
        assert_eq!(value["retryable"], true);
        assert_eq!(value["hint"], "snapshot again");
        assert_eq!(value["suggested_next"].as_array().unwrap().len(), 2);
        assert_eq!(value["suggested_next"][0], "cu snapshot Finder");
        assert_eq!(value["suggested_next"][1], "cu click 3 --app Finder");
        assert_eq!(value["diagnostics"]["ref"], 3);
    }
}
