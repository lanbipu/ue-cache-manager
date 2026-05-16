//! Event types + emitter abstraction. NDJSON for `--json` mode; human-readable otherwise.
//!
//! Event taxonomy matches §8.2 of the design spec.

use crate::error::UecmError;
use serde::Serialize;
use std::io::{self, Write};

/// All events emitted to stdout. Long-running tasks emit one event per stream item.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started {
        task_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        #[serde(skip_serializing_if = "serde_json::Value::is_null")]
        metadata: serde_json::Value,
    },
    HostProbe {
        ip: String,
        winrm_open: bool,
        smb_open: bool,
    },
    Spawned {
        pid: i64,
        log_path: String,
    },
    LogLine {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        parsed_kind: Option<String>,
    },
    Progress {
        #[serde(skip_serializing_if = "Option::is_none")]
        pct: Option<f32>,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        current: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<i64>,
    },
    ItemStarted {
        item_id: String,
        index: i64,
        total: i64,
    },
    ItemCompleted {
        item_id: String,
        index: i64,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Finding {
        rule_id: String,
        severity: String,
        file_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        section: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    Cancelled {
        reason: String,
    },
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "serde_json::Value::is_null")]
        details: serde_json::Value,
    },
    Completed {
        summary: serde_json::Value,
    },
}

/// Map `UecmError` to a stable string code for the `error` event.
///
/// Database / IO failures map to `environment_error` (same family as
/// Configuration) so automation can tell environment problems apart from
/// regular operation failures.
pub fn error_code(err: &UecmError) -> &'static str {
    match err {
        UecmError::InvalidInput(_) => "invalid_input",
        UecmError::OperationFailed(_) => "operation_failed",
        UecmError::PowerShell(_) => "powershell_failed",
        UecmError::Configuration(_) | UecmError::Database(_) | UecmError::Io(_) => {
            "environment_error"
        }
    }
}

/// Process exit code mapping (§6.3 of spec).
///
/// `Database` and `Io` errors share the exit code (3 = environment failure)
/// with `Configuration` — DB unwritable, ps-scripts missing, and similar I/O
/// issues are all "user needs to fix their environment" cases.
pub fn exit_code_for(err: &UecmError) -> i32 {
    match err {
        UecmError::InvalidInput(_) => 2,
        UecmError::Configuration(_) | UecmError::Database(_) | UecmError::Io(_) => 3,
        UecmError::PowerShell(_) => 4,
        UecmError::OperationFailed(_) => 1,
    }
}

/// Object-safe emitter trait.
///
/// `emit_value` takes an already-serialized `serde_json::Value` so the trait
/// remains object-safe — generic methods cannot live on the trait directly or
/// `Box<dyn Emitter>` would not compile. Handlers should call `emit_result`
/// from the `EmitSerialize` extension trait below, which serializes for them.
pub trait Emitter {
    fn emit_event(&mut self, event: &Event) -> io::Result<()>;
    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()>;
    fn emit_error(&mut self, err: &UecmError);
}

/// Convenience generic method available on every `Emitter`, including
/// `Box<dyn Emitter>`. Provided as an extension trait with a blanket impl so
/// the underlying `Emitter` trait stays object-safe.
pub trait EmitSerialize: Emitter {
    fn emit_result<T: Serialize>(&mut self, value: &T) -> io::Result<()> {
        let v = serde_json::to_value(value)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        self.emit_value(&v)
    }
}

impl<E: Emitter + ?Sized> EmitSerialize for E {}

pub struct NdjsonEmitter<W: Write> {
    pub writer: W,
}

impl<W: Write> NdjsonEmitter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Emitter for NdjsonEmitter<W> {
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    fn emit_error(&mut self, err: &UecmError) {
        let ev = Event::Error {
            code: error_code(err).into(),
            message: err.to_string(),
            details: serde_json::Value::Null,
        };
        let _ = self.emit_event(&ev);
    }
}

pub struct HumanEmitter<W: Write, E: Write> {
    pub stdout: W,
    pub stderr: E,
    pub use_color: bool,
}

impl<W: Write, E: Write> HumanEmitter<W, E> {
    pub fn new(stdout: W, stderr: E, use_color: bool) -> Self {
        Self { stdout, stderr, use_color }
    }
}

impl<W: Write, E: Write> Emitter for HumanEmitter<W, E> {
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        match event {
            Event::Started { task_type, .. } => {
                writeln!(self.stderr, "→ starting {}", task_type)?;
            }
            Event::HostProbe { ip, winrm_open, smb_open } => {
                let badges = format!(
                    "winrm={} smb={}",
                    if *winrm_open { "✓" } else { "✗" },
                    if *smb_open { "✓" } else { "✗" }
                );
                writeln!(self.stdout, "  {}  {}", ip, badges)?;
            }
            Event::Spawned { pid, log_path } => {
                writeln!(self.stderr, "→ spawned pid={} log={}", pid, log_path)?;
            }
            Event::LogLine { text, .. } => {
                writeln!(self.stderr, "  | {}", text)?;
            }
            Event::Progress { pct, label, current, total, .. } => {
                let suffix = match (current, total) {
                    (Some(c), Some(t)) => format!(" ({}/{})", c, t),
                    _ => String::new(),
                };
                match pct {
                    Some(p) => writeln!(self.stderr, "→ [{:>5.1}%] {}{}", p * 100.0, label, suffix)?,
                    None => writeln!(self.stderr, "→ {}{}", label, suffix)?,
                }
            }
            Event::ItemStarted { item_id, index, total } => {
                writeln!(self.stderr, "→ [{}/{}] {}", index + 1, total, item_id)?;
            }
            Event::ItemCompleted { item_id, ok, message, .. } => {
                let mark = if *ok { "✓" } else { "✗" };
                let suffix = message.as_deref().unwrap_or("");
                writeln!(self.stderr, "  {} {} {}", mark, item_id, suffix)?;
            }
            Event::Finding { rule_id, severity, file_path, section, key } => {
                writeln!(
                    self.stdout,
                    "  [{}] {} {} :: {} {}",
                    severity, rule_id, file_path,
                    section.as_deref().unwrap_or("-"),
                    key.as_deref().unwrap_or("-"),
                )?;
            }
            Event::Cancelled { reason } => {
                writeln!(self.stderr, "✗ cancelled: {}", reason)?;
            }
            Event::Error { code, message, .. } => {
                writeln!(self.stderr, "✗ error ({}): {}", code, message)?;
            }
            Event::Completed { summary } => {
                writeln!(self.stderr, "✓ done {}", summary)?;
            }
        }
        Ok(())
    }

    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()> {
        // Default human rendering of arbitrary value: pretty JSON to stdout.
        // Individual handlers can take over with custom table rendering.
        let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "<unserializable>".into());
        writeln!(self.stdout, "{}", s)
    }

    fn emit_error(&mut self, err: &UecmError) {
        let _ = writeln!(self.stderr, "✗ error: {}", err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_emits_one_line_per_event() {
        let mut buf = Vec::new();
        {
            let mut emitter = NdjsonEmitter::new(&mut buf);
            emitter
                .emit_event(&Event::HostProbe {
                    ip: "192.168.10.20".into(),
                    winrm_open: true,
                    smb_open: true,
                })
                .unwrap();
            emitter
                .emit_event(&Event::Completed {
                    summary: serde_json::json!({"hosts": 1}),
                })
                .unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = s.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 2);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["kind"], "host_probe");
        assert_eq!(parsed["ip"], "192.168.10.20");
        assert_eq!(parsed["winrm_open"], true);
    }

    #[test]
    fn ndjson_omits_none_fields() {
        let mut buf = Vec::new();
        {
            let mut emitter = NdjsonEmitter::new(&mut buf);
            emitter
                .emit_event(&Event::LogLine {
                    text: "hello".into(),
                    parsed_kind: None,
                })
                .unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(s.trim_end()).unwrap();
        assert!(parsed.get("parsed_kind").is_none());
    }

    #[test]
    fn error_event_uses_stable_code() {
        let err = UecmError::InvalidInput("bad".into());
        assert_eq!(error_code(&err), "invalid_input");
        assert_eq!(exit_code_for(&err), 2);
    }

    #[test]
    fn human_emits_host_probe_with_badges() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        {
            let mut emitter = HumanEmitter::new(&mut stdout, &mut stderr, false);
            emitter
                .emit_event(&Event::HostProbe {
                    ip: "192.168.10.20".into(),
                    winrm_open: true,
                    smb_open: false,
                })
                .unwrap();
        }
        let s = String::from_utf8(stdout).unwrap();
        assert!(s.contains("192.168.10.20"));
        assert!(s.contains("winrm=✓"));
        assert!(s.contains("smb=✗"));
    }
}
