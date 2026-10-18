//! Local bounded preview failure evidence; diagnostic codes remain producer-owned.
use serde_json::{Value, json};
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use toniator_engine::{EvaluationError, SchedulerError};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 32 * 1024;

/// Names the actual app boundary that preserves a diagnostic before display formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    Evaluation,
    Source,
    Texture,
    Scheduler,
    Preparation,
}

/// Holds a bounded exact producer code/message, independent of artist-facing wording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) origin: Origin,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) injected: bool,
}

/// Distinguishes only audited allocator reports from all other unexpected render failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    Unexpected,
    GeometryMemory,
    SourceMemory,
    PreviewMemory,
    Worker,
}

impl Diagnostic {
    /// Captures bounded producer text without parsing it or inferring causes from message suffixes.
    pub(crate) fn new(origin: Origin, code: &str, message: &str) -> Self {
        Self {
            origin,
            code: bounded(code, 256),
            message: bounded(message, 2048),
            injected: false,
        }
    }
    /// Preserves the evaluator's exact path before converting presentation text.
    pub(crate) fn evaluation(error: &EvaluationError) -> Self {
        Self::new(Origin::Evaluation, error.path(), error.message())
    }
    /// Maps every current scheduler lifecycle variant explicitly, without text classification.
    pub(crate) fn scheduler(error: SchedulerError) -> Self {
        let code = match error {
            SchedulerError::WorkerSpawn => "preview.worker.spawn",
            SchedulerError::WorkerUnavailable => "preview.worker.unavailable",
            SchedulerError::WorkerPanicked => "preview.worker.panicked",
            SchedulerError::TicketExhausted => "preview.scheduler.ticket_exhausted",
        };
        Self::new(Origin::Scheduler, code, &error.to_string())
    }
    /// Classifies only exact audited producer codes; limits/numerics/unknown aliases are software failures.
    pub(crate) fn cause(&self) -> Cause {
        match self.code.as_str() {
            "curve.path.allocation" | "coverage.curved_guides.intersections.allocation" => {
                Cause::GeometryMemory
            }
            "source.allocation"
            | "preview.proxy.allocation"
            | "preview.source_display.allocation" => Cause::SourceMemory,
            "raster.buffer.allocation" | "preview.texture.allocation" => Cause::PreviewMemory,
            "preview.bridge.spawn"
            | "preview.worker.spawn"
            | "preview.worker.unavailable"
            | "preview.worker.panicked"
            | "preview.source_worker.unavailable"
            | "preview.source_worker.panicked"
            | "preview.source_worker.spawn" => Cause::Worker,
            _ => Cause::Unexpected,
        }
    }
    /// Provides short truthful cause text without suggesting artwork edits for implementation failures.
    pub(crate) fn summary(&self) -> &'static str {
        match self.cause() {
            Cause::GeometryMemory => "Geometry needs more memory.",
            Cause::SourceMemory => "Source preview needs more memory.",
            Cause::PreviewMemory => "Preview pixels need more memory.",
            Cause::Worker => "The preview worker stopped.",
            Cause::Unexpected => "A software rendering error occurred.",
        }
    }
    /// Offers only optional causal reductions appropriate to the allocation's real workload.
    pub(crate) fn suggestions(&self) -> &'static [&'static str] {
        match self.cause() {
            Cause::GeometryMemory => &[
                "Increasing Feature size may reduce generated geometry, with a coarser appearance; it does not simplify authored guides.",
                "For complex authored guides, simplify a copy of the guide or free memory before retrying.",
            ],
            Cause::SourceMemory => {
                &["Try a smaller copy of the source artwork, or free memory before retrying."]
            }
            Cause::PreviewMemory => &["Free memory before retrying this preview."],
            Cause::Worker | Cause::Unexpected => &[],
        }
    }
    /// Recognizes typed cancellation only; it never becomes a logged terminal failure.
    pub(crate) fn is_cancelled(&self) -> bool {
        matches!(
            self.code.as_str(),
            "evaluation.cancelled" | "source.cancelled"
        )
    }
    /// Identifies the affected recreatable worker without conflating ordinary render errors.
    pub(crate) fn source_worker_failed(&self) -> bool {
        matches!(
            self.code.as_str(),
            "preview.source_worker.unavailable"
                | "preview.source_worker.panicked"
                | "preview.source_worker.spawn"
        )
    }
}
impl fmt::Display for Diagnostic {
    /// Formats preserved diagnostic authority for tooltips and legacy status consumers only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Keeps immutable technical details and the real local log result pinned across newer failures.
#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub(crate) diagnostic: Diagnostic,
    pub(crate) context: Value,
    pub(crate) timestamp: u64,
    pub(crate) log_path: Option<PathBuf>,
    pub(crate) log_outcome: String,
}
impl Snapshot {
    /// Records one already freshness-admitted failure; IO never changes rendering or recovery.
    pub(crate) fn record(diagnostic: Diagnostic, context: Value) -> Arc<Self> {
        let path = crate::default_recent_file_path()
            .map(|p| p.with_file_name("preview-failures.log"))
            .ok_or("Default state directory unavailable");
        Self::record_with_path(diagnostic, context, path)
    }
    /// Records focused failure witnesses only at a task-owned path without process-global XDG changes.
    #[cfg(test)]
    pub(crate) fn record_at(diagnostic: Diagnostic, context: Value, path: &Path) -> Arc<Self> {
        Self::record_with_path(diagnostic, context, Ok(path.to_owned()))
    }
    /// Bounds metadata and appends one immutable snapshot through the same production log writer.
    /// Path/IO errors remain local evidence outcomes and never alter caller rendering state.
    fn record_with_path(
        diagnostic: Diagnostic,
        context: Value,
        path: Result<PathBuf, &str>,
    ) -> Arc<Self> {
        let context = if serde_json::to_writer(ContextCounter(0), &context).is_err() {
            json!({"metadata_truncated":true})
        } else {
            context
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut snapshot = Self {
            diagnostic,
            context,
            timestamp,
            log_path: path.as_ref().ok().cloned(),
            log_outcome: "Not written".into(),
        };
        snapshot.log_outcome = match path {
            Ok(path) => match append_record(&path, &snapshot.entry(), MAX_LOG_BYTES) {
                Ok(()) => "Recorded locally".into(),
                Err(error) => format!(
                    "Could not write local log: {}",
                    bounded(&error.to_string(), 512)
                ),
            },
            Err(error) => format!("Local log path unavailable: {}", bounded(error, 512)),
        };
        Arc::new(snapshot)
    }
    /// Projects one bounded versioned record with no source bytes, artwork name or recipe serialization.
    fn entry(&self) -> Value {
        json!({"schema":1,"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"timestamp_unix_seconds":self.timestamp,"origin":format!("{:?}",self.diagnostic.origin),"code":self.diagnostic.code,"message":self.diagnostic.message,"injected_routing_witness":self.diagnostic.injected,"context":self.context})
    }
    /// Derives causal memory suggestions from captured surface and exact allocation producer.
    pub(crate) fn suggestions(&self) -> &'static [&'static str] {
        if self.diagnostic.cause() == Cause::SourceMemory
            && self.context["surface"] == "Advanced Settings"
        {
            return &["Free memory, then close and reopen Advanced Settings to retry source preparation. Editing settings does not rerun it."];
        }
        if self.diagnostic.code == "preview.proxy.allocation" {
            return &["Free memory before retrying this fixed-size private source preview."];
        }
        if self.diagnostic.cause() == Cause::PreviewMemory && self.context["surface"] == "Main" {
            return &["Try a smaller preview window, or free memory before retrying."];
        }
        self.diagnostic.suggestions()
    }
    /// Copies the exact captured failure, context and log outcome rather than reading mutable app state.
    pub(crate) fn details(&self) -> String {
        let recovery = if self.diagnostic.cause() == Cause::Worker {
            if self.context["surface"] == "Main" {
                "Retry preview creates a fresh affected worker."
            } else {
                "Close and reopen this private preview to create a fresh worker."
            }
        } else {
            "The artwork and settings have not been changed. Keep these details for reporting unexpected rendering failures."
        };
        format!(
            "{}\n{}\n\n{}\n\nLog file: {}\nLog outcome: {}\n\nLocal only. Technical messages may contain local paths; review before sharing.",
            self.diagnostic.summary(),
            recovery,
            serde_json::to_string_pretty(&self.entry())
                .unwrap_or_else(|_| "Technical snapshot unavailable".into()),
            self.log_path
                .as_ref()
                .map_or_else(|| "Unavailable".into(), |p| p.display().to_string()),
            self.log_outcome
        )
    }
}

/// Counts JSON bytes without allocating a serialized copy of oversized caller context.
struct ContextCounter(usize);
impl Write for ContextCounter {
    /// Rejects metadata beyond its bound before entry construction can clone it.
    /// # Errors
    /// Returns InvalidData when serialized metadata exceeds8192bytes or the counter overflows.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= 8192)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "context exceeds bound"))?;
        Ok(bytes.len())
    }
    /// Has no buffered output or IO side effects; flushing always succeeds.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Truncates UTF8 at a character boundary with a visible marker and a strict byte maximum.
pub(crate) fn bounded(text: &str, maximum: usize) -> String {
    if text.len() <= maximum {
        return text.to_owned();
    }
    if maximum < 3 {
        return ".".repeat(maximum);
    }
    let mut end = maximum.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Appends bounded UTF8 NDJSON, rotating at most two local backups before exceeding the current bound.
/// # Errors
/// Returns bounded-record or filesystem diagnostics; callers must keep this best effort.
fn append_record(path: &Path, entry: &Value, maximum: u64) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(entry)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_ENTRY_BYTES || bytes.len() as u64 > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preview failure record exceeds its bound",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let length = regular_log_length(path)?;
    if length.saturating_add(bytes.len() as u64) > maximum {
        let backup1 = path.with_file_name(format!(
            "{}.1",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        let backup2 = path.with_file_name(format!(
            "{}.2",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        regular_log_length(&backup1)?;
        regular_log_length(&backup2)?;
        match fs::remove_file(&backup2) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        match fs::rename(&backup1, &backup2) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        match fs::rename(path, &backup1) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&bytes)
}

/// Admits existing regular log files only, without following symlinks or moving directories.
/// # Errors
/// Returns an IO diagnostic for unknown path kinds before any rotation mutation.
fn regular_log_length(path: &Path) -> io::Result<u64> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(metadata.len()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local log path is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

/// Retains bridge identity in queued callbacks, preventing ticket1 collisions and allocator-address reuse.
pub(crate) fn same_bridge(current: &Arc<()>, candidate: &Arc<()>) -> bool {
    Arc::ptr_eq(current, candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies exact actual-allocation classification and rejection of guessed OOM causes.
    /// # Panics
    /// Panics if counts/numerics/unknown aliases receive artwork reduction advice.
    #[test]
    fn exact_producer_causes_and_bridge_identity() {
        for code in [
            "raster.allocation",
            "curve.path.count_overflow",
            "coverage.curved_guides.intersections.addressability",
            "raster.flatten.numeric",
            "source.decode",
            "unknown.allocation",
        ] {
            let d = Diagnostic::new(Origin::Evaluation, code, "out of memory");
            assert_eq!(d.cause(), Cause::Unexpected);
            assert!(d.suggestions().is_empty());
        }
        for (code, cause) in [
            ("curve.path.allocation", Cause::GeometryMemory),
            ("source.allocation", Cause::SourceMemory),
            ("raster.buffer.allocation", Cause::PreviewMemory),
        ] {
            assert_eq!(Diagnostic::new(Origin::Evaluation, code, "").cause(), cause);
        }
        let old = Arc::new(());
        let queued = old.clone();
        let fresh = Arc::new(());
        assert!(same_bridge(&old, &queued));
        assert!(!same_bridge(&fresh, &queued));
    }
    /// Keeps guidance causal to captured fixed/main allocations and technical snapshots immutable.
    /// # Panics
    /// Panics if a fixed private target receives ineffective window advice or copied details drift.
    #[test]
    fn captured_surface_guidance_and_immutable_details() {
        let mut context = json!({"surface":"Main"});
        let snapshot = Snapshot {
            diagnostic: Diagnostic::new(
                Origin::Texture,
                "preview.texture.allocation",
                "first failure",
            ),
            context: context.clone(),
            timestamp: 7,
            log_path: Some(PathBuf::from("local.log")),
            log_outcome: "Could not write local log: read only".into(),
        };
        let copied = snapshot.details();
        context["surface"] = "Advanced Settings".into();
        assert!(snapshot.suggestions()[0].contains("smaller preview window"));
        let private = Snapshot {
            context,
            ..snapshot.clone()
        };
        assert!(!private.suggestions()[0].contains("window"));
        assert_eq!(copied, snapshot.details());
        assert!(copied.contains("read only"));
        assert!(
            serde_json::to_writer(ContextCounter(0), &json!({"too_large":"x".repeat(9000)}))
                .is_err()
        );
    }
    /// Parses every rotated NDJSON record and proves bounded backups and nonfatal IO outcomes.
    /// # Panics
    /// Panics if log bounds/rotation/schema or real IO failure reporting change.
    #[test]
    fn bounded_ndjson_rotation_and_io_failure() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/validation/preview-reliability-20261018/app/log-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("preview-failures.log");
        let entry = json!({"schema":1,"message":"α\nlocal"});
        for _ in 0..12 {
            append_record(&path, &entry, 90).unwrap();
        }
        for name in [
            "preview-failures.log",
            "preview-failures.log.1",
            "preview-failures.log.2",
        ] {
            let p = root.join(name);
            assert!(p.metadata().unwrap().len() <= 90);
            for line in fs::read_to_string(p).unwrap().lines() {
                assert_eq!(serde_json::from_str::<Value>(line).unwrap()["schema"], 1);
            }
        }
        fs::write(
            root.join("directory-marker"),
            b"preserve directory contents",
        )
        .unwrap();
        assert!(append_record(&root, &entry, 90).is_err());
        assert!(root.is_dir());
        assert_eq!(
            fs::read(root.join("directory-marker")).unwrap(),
            b"preserve directory contents"
        );
        let blocked = root.join("backup-directory.log");
        fs::write(&blocked, b"old regular log").unwrap();
        let backup = root.join("backup-directory.log.1");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("marker"), b"preserve backup directory").unwrap();
        assert!(append_record(&blocked, &entry, 40).is_err());
        assert_eq!(fs::read(&blocked).unwrap(), b"old regular log");
        assert_eq!(
            fs::read(backup.join("marker")).unwrap(),
            b"preserve backup directory"
        );
        #[cfg(unix)]
        {
            let symlink = root.join("symlink.log");
            std::os::unix::fs::symlink(&blocked, &symlink).unwrap();
            assert!(append_record(&symlink, &entry, 40).is_err());
            assert!(
                fs::symlink_metadata(&symlink)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(fs::read(&blocked).unwrap(), b"old regular log");
        }
        assert!(
            append_record(
                &path,
                &json!({"message":"x".repeat(MAX_ENTRY_BYTES)}),
                MAX_LOG_BYTES
            )
            .is_err()
        );
        assert!(bounded(&"α".repeat(100), 31).len() <= 31);
        for n in 0..3 {
            assert!(bounded("αabcdef", n).len() <= n);
        }
    }
}

/// Captures bounded current authority metadata without source bytes/hashes, paths/names or recipe text.
/// Authored counts describe persisted input, never estimated generated geometry.
pub(crate) fn context(
    surface: &str,
    document: &toniator_domain::Document,
    source: Option<&toniator_engine::SourceIdentity>,
    revision: u64,
    frame: u64,
    target: Option<toniator_engine::PreviewRasterTarget>,
    retained_pixels: bool,
) -> Value {
    let channels:Vec<_>=document.channel_topology().map_or_else(Vec::new,|topology|topology.channels().iter().take(8).map(|channel|{
        let definition=document.pattern_definition_for(channel.id);
        json!({"channel_id":channel.id.0,"definition_id":definition.map(|d|d.id.0),"family":definition.map(|d|bounded(&format!("{:?}",d.family),512)),"mechanisms":definition.map(|d|d.mechanisms.len()),"output_layers":definition.map(|d|d.output_layers.len())})
    }).collect());
    json!({"surface":bounded(surface,64),"version":env!("CARGO_PKG_VERSION"),"revision":revision,"frame":frame,"target":target.map(|t|bounded(&format!("{t:?}"),512)),"retained_pixels":retained_pixels,"source":source.map(|s|json!({"format":format!("{:?}",s.format),"width":s.width,"height":s.height})),"canvas":[document.canvas().width,document.canvas().height],"model":format!("{:?}",document.channel_model()),"channels":channels,"channel_metadata_truncated":document.channel_topology().is_some_and(|t|t.channels().len()>8),"authored_structures":document.authored_structures().len(),"authored_segments":document.authored_structures().iter().map(|s|s.segments().len()).sum::<usize>()})
}
