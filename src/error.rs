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

#[derive(Clone, Copy, Debug)]
pub enum ErrorCode {
    CommandFailed,
    InvalidArgument,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::CommandFailed => "command_failed",
            Self::InvalidArgument => "invalid_argument",
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
            "schema_version": crate::protocol::MACHINE_SCHEMA_VERSION,
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
        Self::msg(s)
    }
}

impl From<&str> for CuError {
    fn from(s: &str) -> Self {
        Self::msg(s.to_string())
    }
}
