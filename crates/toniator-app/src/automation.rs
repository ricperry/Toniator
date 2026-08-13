//! Opt-in, observational JSONL evidence for deterministic GUI checks.

use std::{env, fs, io::Write};

/// Writes immutable app-state records without offering a document-control API.
pub(crate) struct AutomationSink {
    file: fs::File,
}

impl AutomationSink {
    /// Opens the configured evidence file or disables observation on failure.
    pub(crate) fn from_environment() -> Option<Self> {
        let path = env::var_os("TONIATOR_AUTOMATION_EVENTS")?;
        match fs::OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => Some(Self { file }),
            Err(error) => {
                eprintln!("toniator-app: automation evidence is unavailable: {error}");
                None
            }
        }
    }

    /// Appends and flushes exactly one valid immutable JSON record.
    pub(crate) fn emit(&mut self, event: &serde_json::Value) {
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        let _ = self
            .file
            .write_all(line.as_bytes())
            .and_then(|_| self.file.write_all(b"\n"))
            .and_then(|_| self.file.flush());
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    /// Writes ordered, newline-delimited JSON records and flushes before return.
    ///
    /// The temporary file is local test evidence only; the test neither reads
    /// application authority nor exposes an automation control surface.
    #[test]
    fn sink_emits_parseable_jsonl_in_call_order() {
        let path = std::env::temp_dir().join(format!(
            "toniator-automation-{}-{}.jsonl",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .expect("temporary evidence file opens");
        let mut sink = AutomationSink { file };
        sink.emit(&serde_json::json!({"event":"first","workspace_generation":1}));
        sink.emit(&serde_json::json!({"event":"second","workspace_generation":1}));
        let content = fs::read_to_string(&path).expect("flushed records are readable");
        let records = content
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["event"], "first");
        assert_eq!(records[1]["event"], "second");
        fs::remove_file(path).expect("temporary evidence file is removable");
    }
}
