//! Debug and trace contracts for Freehand observation paths.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

use freehand_contracts::{AgentId, FeatureId, SessionId, TraceId, TurnId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugSemanticPosition {
    pub feature_id: FeatureId,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub agent_id: Option<AgentId>,
    pub pipeline_node: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugScenePosition {
    pub crate_name: String,
    pub file: String,
    pub function: String,
    pub line: Option<u32>,
    pub artifact_path: Option<String>,
    pub raw_exchange_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugTraceEnvelope {
    pub semantic: DebugSemanticPosition,
    pub scene: DebugScenePosition,
    pub input_hash: Option<String>,
    pub output_hash: Option<String>,
    pub artifact_path: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugStateSnapshot {
    pub semantic: DebugSemanticPosition,
    pub scene: DebugScenePosition,
    pub status_text: String,
    pub detail_lines: Vec<String>,
}

impl DebugStateSnapshot {
    pub fn new(
        semantic: DebugSemanticPosition,
        scene: DebugScenePosition,
        status_text: impl Into<String>,
        detail_lines: Vec<String>,
    ) -> Self {
        Self {
            semantic,
            scene,
            status_text: status_text.into(),
            detail_lines,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugSinkKind {
    MemorySubscriber,
    Stdout,
    FileLedger,
    ReplayCapture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugEvent {
    pub envelope: DebugTraceEnvelope,
    pub snapshot: Option<DebugStateSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugObservationFailure {
    pub sink_kind: DebugSinkKind,
    pub event_envelope: DebugTraceEnvelope,
    pub message: String,
}

pub trait DebugSink: Send + Sync {
    fn kind(&self) -> DebugSinkKind;
    fn handle(&self, event: &DebugEvent) -> Result<(), DebugSinkError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DebugSinkError {
    #[error("io failure: {0}")]
    Io(String),
    #[error("encode failure: {0}")]
    Encode(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DebugHubError {
    #[error("debug sink `{kind:?}` failed: {message}")]
    SinkDispatch {
        kind: DebugSinkKind,
        message: String,
    },
}

pub struct DebugHub {
    enabled: AtomicBool,
    subscribers: Mutex<Vec<SyncSender<DebugEvent>>>,
    failure_subscribers: Mutex<Vec<SyncSender<DebugObservationFailure>>>,
    sinks: Mutex<Vec<Arc<dyn DebugSink>>>,
}

impl DebugHub {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            subscribers: Mutex::new(Vec::new()),
            failure_subscribers: Mutex::new(Vec::new()),
            sinks: Mutex::new(Vec::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn subscribe(&self, capacity: usize) -> Receiver<DebugEvent> {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        self.subscribers
            .lock()
            .expect("lock debug subscribers")
            .push(sender);
        receiver
    }

    pub fn subscribe_failures(&self, capacity: usize) -> Receiver<DebugObservationFailure> {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        self.failure_subscribers
            .lock()
            .expect("lock debug failure subscribers")
            .push(sender);
        receiver
    }

    pub fn add_sink<S>(&self, sink: S)
    where
        S: DebugSink + 'static,
    {
        self.sinks
            .lock()
            .expect("lock debug sinks")
            .push(Arc::new(sink));
    }

    pub fn emit(&self, event: DebugEvent) -> Result<(), DebugHubError> {
        if !self.is_enabled() {
            return Ok(());
        }

        {
            let mut subscribers = self.subscribers.lock().expect("lock debug subscribers");
            subscribers.retain(|sender| match sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
        }

        let sinks = self.sinks.lock().expect("lock debug sinks");
        for sink in sinks.iter() {
            if let Err(err) = sink.handle(&event) {
                let failure = DebugObservationFailure {
                    sink_kind: sink.kind(),
                    event_envelope: event.envelope.clone(),
                    message: err.to_string(),
                };
                self.publish_failure(failure);
                return Err(DebugHubError::SinkDispatch {
                    kind: sink.kind(),
                    message: err.to_string(),
                });
            }
        }
        Ok(())
    }

    fn publish_failure(&self, failure: DebugObservationFailure) {
        let mut subscribers = self
            .failure_subscribers
            .lock()
            .expect("lock debug failure subscribers");
        subscribers.retain(|sender| match sender.try_send(failure.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
}

pub struct StdoutDebugSink;

impl DebugSink for StdoutDebugSink {
    fn kind(&self) -> DebugSinkKind {
        DebugSinkKind::Stdout
    }

    fn handle(&self, event: &DebugEvent) -> Result<(), DebugSinkError> {
        let line =
            serde_json::to_string(event).map_err(|err| DebugSinkError::Encode(err.to_string()))?;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{line}").map_err(|err| DebugSinkError::Io(err.to_string()))
    }
}

pub struct FileDebugSink {
    path: PathBuf,
}

impl FileDebugSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl DebugSink for FileDebugSink {
    fn kind(&self) -> DebugSinkKind {
        DebugSinkKind::FileLedger
    }

    fn handle(&self, event: &DebugEvent) -> Result<(), DebugSinkError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| DebugSinkError::Io(err.to_string()))?;
        }
        let encoded =
            serde_json::to_string(event).map_err(|err| DebugSinkError::Encode(err.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| DebugSinkError::Io(err.to_string()))?;
        writeln!(file, "{encoded}").map_err(|err| DebugSinkError::Io(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FailingDebugSink;

    impl DebugSink for FailingDebugSink {
        fn kind(&self) -> DebugSinkKind {
            DebugSinkKind::ReplayCapture
        }

        fn handle(&self, _event: &DebugEvent) -> Result<(), DebugSinkError> {
            Err(DebugSinkError::Io("debug sink failed".to_owned()))
        }
    }

    fn semantic_position() -> DebugSemanticPosition {
        DebugSemanticPosition {
            feature_id: FeatureId::new("debug.core"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            agent_id: Some(AgentId::new("agent-1")),
            pipeline_node: Some("ReasonResp01SemanticEvent".to_owned()),
        }
    }

    fn scene_position() -> DebugScenePosition {
        DebugScenePosition {
            crate_name: "freehand-debug".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: "tests::scene_position".to_owned(),
            line: Some(42),
            artifact_path: Some("~/.freehand/ledgers/debug/core.jsonl".to_owned()),
            raw_exchange_id: Some("raw-1".to_owned()),
        }
    }

    #[test]
    fn debug_snapshot_preserves_semantic_and_scene_positions() {
        let snapshot = DebugStateSnapshot::new(
            semantic_position(),
            scene_position(),
            "debug snapshot ready",
            vec!["line one".to_owned(), "line two".to_owned()],
        );

        assert_eq!(snapshot.semantic.feature_id, FeatureId::new("debug.core"));
        assert_eq!(snapshot.semantic.turn_id, TurnId::new("turn-1"));
        assert_eq!(snapshot.scene.crate_name, "freehand-debug");
        assert_eq!(snapshot.status_text, "debug snapshot ready");
        assert_eq!(snapshot.detail_lines, vec!["line one", "line two"]);
    }

    #[test]
    fn trace_envelope_round_trips_as_json() {
        let envelope = DebugTraceEnvelope {
            semantic: semantic_position(),
            scene: scene_position(),
            input_hash: Some("input-hash".to_owned()),
            output_hash: Some("output-hash".to_owned()),
            artifact_path: Some("~/.freehand/replays/debug/core.json".to_owned()),
            timestamp: "2026-06-16T00:00:00Z".to_owned(),
        };

        let encoded = serde_json::to_string(&envelope).expect("encode");
        let decoded: DebugTraceEnvelope = serde_json::from_str(&encoded).expect("decode");

        assert_eq!(decoded, envelope);
    }

    fn debug_event() -> DebugEvent {
        let snapshot = DebugStateSnapshot::new(
            semantic_position(),
            scene_position(),
            "debug snapshot ready",
            vec!["line one".to_owned(), "line two".to_owned()],
        );
        DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic.clone(),
                scene: snapshot.scene.clone(),
                input_hash: Some("input-hash".to_owned()),
                output_hash: Some("output-hash".to_owned()),
                artifact_path: snapshot.scene.artifact_path.clone(),
                timestamp: "2026-06-16T00:00:00Z".to_owned(),
            },
            snapshot: Some(snapshot),
        }
    }

    #[test]
    fn hub_fans_out_to_subscribers() {
        let hub = DebugHub::new(true);
        let receiver = hub.subscribe(2);
        hub.emit(debug_event()).expect("emit");

        let received = receiver.recv().expect("receive");
        assert_eq!(
            received.snapshot.expect("snapshot").status_text,
            "debug snapshot ready"
        );
    }

    #[test]
    fn hub_dispatches_to_file_sink() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("freehand-debug-{unique}.jsonl"));
        let hub = DebugHub::new(true);
        hub.add_sink(FileDebugSink::new(&path));
        hub.emit(debug_event()).expect("emit");

        let stored = fs::read_to_string(&path).expect("read");
        assert!(stored.contains("\"status_text\":\"debug snapshot ready\""));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn disabled_hub_drops_delivery() {
        let hub = DebugHub::new(false);
        let receiver = hub.subscribe(1);
        hub.emit(debug_event()).expect("emit");
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn hub_surfaces_sink_failure_through_observation_failure_stream() {
        let hub = DebugHub::new(true);
        let receiver = hub.subscribe(1);
        let failure_receiver = hub.subscribe_failures(1);
        hub.add_sink(FailingDebugSink);

        let err = hub
            .emit(debug_event())
            .expect_err("sink failure must surface");

        assert_eq!(
            err,
            DebugHubError::SinkDispatch {
                kind: DebugSinkKind::ReplayCapture,
                message: "io failure: debug sink failed".to_owned(),
            }
        );
        let delivered = receiver.recv().expect("subscriber event");
        assert_eq!(
            delivered.envelope.semantic.feature_id,
            FeatureId::new("debug.core")
        );

        let failure = failure_receiver.recv().expect("failure event");
        assert_eq!(failure.sink_kind, DebugSinkKind::ReplayCapture);
        assert_eq!(
            failure.event_envelope.semantic.turn_id,
            TurnId::new("turn-1")
        );
        assert_eq!(failure.message, "io failure: debug sink failed");
    }
}
