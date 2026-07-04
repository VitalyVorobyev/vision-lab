//! Recorder component runtime.

use async_trait::async_trait;
use comm_core::{
    ApiError, CommandReceipt, ComponentIdentity, OperationId, Versioned, event, now, versioned,
};
use comm_local::{
    CommandClient, CommandInbox, EventBus, MonotonicCounter, StateCell, command_channel,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{fs, sync::watch};
use vision_contracts::{
    CameraApi, Frame, RecorderApi, RecorderCommand, RecorderCommandKind, RecorderEvent,
    RecorderEventStream, RecorderLifecycle, RecorderState, VisionApi, VisionEvent,
};

#[derive(Clone)]
pub struct RecorderComponent {
    client: CommandClient<RecorderCommand, CommandReceipt>,
    state: Arc<StateCell<Versioned<RecorderState>>>,
    events: Arc<EventBus<RecorderEvent>>,
}

impl RecorderComponent {
    pub async fn spawn(
        component_name: &str,
        camera: Arc<dyn CameraApi>,
        vision: Arc<dyn VisionApi>,
        base_dir: PathBuf,
    ) -> Result<Arc<Self>, ApiError> {
        let identity =
            ComponentIdentity::new("recorder", component_name, env!("CARGO_PKG_VERSION"));
        let initial = versioned(identity.clone(), 0, RecorderState::default());
        let state = Arc::new(StateCell::new(initial));
        let events = Arc::new(EventBus::new(256));
        let (client, inbox) = command_channel(32);
        let frame_rx = camera.subscribe_frames().await?;
        let vision_events = vision.subscribe().await?;
        let component = Arc::new(Self {
            client,
            state: state.clone(),
            events: events.clone(),
        });
        tokio::spawn(run_recorder(RecorderRuntimeParts {
            identity,
            inbox,
            state_cell: state,
            events,
            frame_rx,
            vision_events,
            camera,
            vision,
            base_dir,
        }));
        Ok(component)
    }
}

#[async_trait]
impl RecorderApi for RecorderComponent {
    async fn submit(&self, command: RecorderCommand) -> Result<CommandReceipt, ApiError> {
        self.client.submit(command).await
    }

    async fn get_state(&self) -> Result<Versioned<RecorderState>, ApiError> {
        Ok(self.state.get().await)
    }

    async fn subscribe(&self) -> Result<RecorderEventStream, ApiError> {
        Ok(self.events.subscribe())
    }
}

struct Runtime {
    identity: ComponentIdentity,
    state_cell: Arc<StateCell<Versioned<RecorderState>>>,
    events: Arc<EventBus<RecorderEvent>>,
    camera: Arc<dyn CameraApi>,
    vision: Arc<dyn VisionApi>,
    base_dir: PathBuf,
    sequence: MonotonicCounter,
    revision: MonotonicCounter,
    state: RecorderState,
    max_fps: f32,
    last_recorded_at: Option<Instant>,
}

struct RecorderRuntimeParts {
    identity: ComponentIdentity,
    inbox: CommandInbox<RecorderCommand, CommandReceipt>,
    state_cell: Arc<StateCell<Versioned<RecorderState>>>,
    events: Arc<EventBus<RecorderEvent>>,
    frame_rx: watch::Receiver<Option<Arc<Frame>>>,
    vision_events: comm_core::EventStream<VisionEvent>,
    camera: Arc<dyn CameraApi>,
    vision: Arc<dyn VisionApi>,
    base_dir: PathBuf,
}

async fn run_recorder(parts: RecorderRuntimeParts) {
    let RecorderRuntimeParts {
        identity,
        mut inbox,
        state_cell,
        events,
        mut frame_rx,
        mut vision_events,
        camera,
        vision,
        base_dir,
    } = parts;

    let mut runtime = Runtime {
        identity,
        state_cell,
        events,
        camera,
        vision,
        base_dir,
        sequence: MonotonicCounter::default(),
        revision: MonotonicCounter::default(),
        state: RecorderState::default(),
        max_fps: 10.0,
        last_recorded_at: None,
    };

    loop {
        tokio::select! {
            changed = frame_rx.changed() => {
                if changed.is_err() {
                    runtime.set_error("camera frame stream closed").await;
                    break;
                }
                let frame = frame_rx.borrow().clone();
                if let Some(frame) = frame {
                    runtime.record_frame(frame).await;
                }
            }
            event_result = vision_events.recv() => {
                match event_result {
                    Ok(event) => runtime.record_vision_event(event).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        runtime.state.dropped_frames = runtime.state.dropped_frames.saturating_add(skipped);
                        runtime.store().await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            Some(request) = inbox.recv() => {
                let result = runtime.handle_command(&request.command).await;
                let _ = request.respond_to.send(result);
            }
            else => break,
        }
    }
}

impl Runtime {
    async fn handle_command(
        &mut self,
        command: &RecorderCommand,
    ) -> Result<CommandReceipt, ApiError> {
        match command.kind {
            RecorderCommandKind::StartRecording { max_fps } => {
                if max_fps <= 0.0 || max_fps > 120.0 {
                    return Err(ApiError::InvalidRequest(
                        "recorder max_fps must be in the range 0..120".into(),
                    ));
                }
                if self.state.lifecycle == RecorderLifecycle::Recording {
                    return Err(ApiError::Rejected("recorder is already running".into()));
                }
                self.max_fps = max_fps;
                let session_dir = self.session_dir();
                fs::create_dir_all(session_dir.join("frames"))
                    .await
                    .map_err(io_error)?;
                self.write_manifest(&session_dir).await?;
                self.state.lifecycle = RecorderLifecycle::Recording;
                self.state.session_path = Some(session_dir.display().to_string());
                self.state.recorded_frames = 0;
                self.state.recorded_detections = 0;
                self.state.dropped_frames = 0;
                self.state.error = None;
                self.last_recorded_at = None;
                self.publish(
                    command.correlation_id,
                    RecorderEvent::SessionStarted {
                        path: session_dir.display().to_string(),
                    },
                );
            }
            RecorderCommandKind::StopRecording => {
                if self.state.lifecycle == RecorderLifecycle::Recording {
                    let path = self.state.session_path.clone().unwrap_or_default();
                    self.state.lifecycle = RecorderLifecycle::Idle;
                    self.publish(
                        command.correlation_id,
                        RecorderEvent::SessionStopped { path },
                    );
                }
            }
        }

        let accepted_revision = self.store().await;
        Ok(CommandReceipt {
            command_id: command.command_id,
            operation_id: Some(OperationId::new()),
            accepted_revision,
        })
    }

    async fn record_frame(&mut self, frame: Arc<Frame>) {
        if self.state.lifecycle != RecorderLifecycle::Recording {
            return;
        }
        if !self.should_record_now() {
            self.state.dropped_frames = self.state.dropped_frames.saturating_add(1);
            self.store().await;
            return;
        }
        let Some(session) = self.session_path() else {
            return;
        };
        let path = session
            .join("frames")
            .join(format!("frame_{:08}.pgm", frame.meta.frame_id));
        if let Err(error) = write_pgm(&path, &frame).await {
            self.set_error(&format!("failed to write frame: {error}"))
                .await;
            return;
        }
        let record = FrameRecord {
            frame_id: frame.meta.frame_id,
            width: frame.meta.width,
            height: frame.meta.height,
            path: path.display().to_string(),
        };
        if let Err(error) = append_jsonl(&session.join("frames.jsonl"), &record).await {
            self.set_error(&format!("failed to write frame metadata: {error}"))
                .await;
            return;
        }
        self.state.recorded_frames = self.state.recorded_frames.saturating_add(1);
        self.publish(
            None,
            RecorderEvent::FrameRecorded {
                frame_id: frame.meta.frame_id,
            },
        );
        self.store().await;
    }

    async fn record_vision_event(&mut self, event: comm_core::EventEnvelope<VisionEvent>) {
        if self.state.lifecycle != RecorderLifecycle::Recording {
            return;
        }
        let Some(session) = self.session_path() else {
            return;
        };
        if let Err(error) = append_jsonl(&session.join("vision-events.jsonl"), &event).await {
            self.set_error(&format!("failed to write vision event: {error}"))
                .await;
            return;
        }
        if let VisionEvent::DetectionProduced { detection } = &event.payload {
            if let Err(error) = append_jsonl(&session.join("detections.jsonl"), detection).await {
                self.set_error(&format!("failed to write detection: {error}"))
                    .await;
                return;
            }
            self.state.recorded_detections = self.state.recorded_detections.saturating_add(1);
            self.publish(
                event.correlation_id,
                RecorderEvent::DetectionRecorded {
                    frame_id: detection.frame_id,
                },
            );
            self.store().await;
        }
    }

    async fn write_manifest(&self, session_dir: &Path) -> Result<(), ApiError> {
        let manifest = Manifest {
            app_version: env!("CARGO_PKG_VERSION"),
            created_at_ms: now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            camera_state: self.camera.get_state().await.ok(),
            vision_state: self.vision.get_state().await.ok(),
        };
        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| ApiError::Failed(error.to_string()))?;
        fs::write(session_dir.join("manifest.json"), json)
            .await
            .map_err(io_error)?;
        Ok(())
    }

    fn should_record_now(&mut self) -> bool {
        let min_interval = Duration::from_secs_f32(1.0 / self.max_fps);
        let now = Instant::now();
        match self.last_recorded_at {
            Some(last) if now.duration_since(last) < min_interval => false,
            _ => {
                self.last_recorded_at = Some(now);
                true
            }
        }
    }

    fn session_dir(&self) -> PathBuf {
        self.base_dir.join(format!("session-{}", unix_millis()))
    }

    fn session_path(&self) -> Option<PathBuf> {
        self.state.session_path.as_ref().map(PathBuf::from)
    }

    async fn set_error(&mut self, message: &str) {
        self.state.lifecycle = RecorderLifecycle::Error;
        self.state.error = Some(message.to_string());
        self.publish(
            None,
            RecorderEvent::Error {
                message: message.to_string(),
            },
        );
        self.store().await;
    }

    fn publish(&self, correlation_id: Option<comm_core::CorrelationId>, payload: RecorderEvent) {
        self.events.publish(event(
            self.identity.clone(),
            self.sequence.advance(),
            correlation_id,
            payload,
        ));
    }

    async fn store(&mut self) -> u64 {
        let next_revision = self.revision.advance();
        self.state_cell
            .set(versioned(
                self.identity.clone(),
                next_revision,
                self.state.clone(),
            ))
            .await;
        next_revision
    }
}

#[derive(Serialize)]
struct Manifest {
    app_version: &'static str,
    created_at_ms: u64,
    camera_state: Option<Versioned<vision_contracts::CameraState>>,
    vision_state: Option<Versioned<vision_contracts::VisionState>>,
}

#[derive(Serialize)]
struct FrameRecord {
    frame_id: u64,
    width: u32,
    height: u32,
    path: String,
}

async fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), std::io::Error> {
    use tokio::io::AsyncWriteExt;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    let line = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    file.write_all(&line).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

async fn write_pgm(path: &Path, frame: &Frame) -> Result<(), std::io::Error> {
    use tokio::io::AsyncWriteExt;

    let mut file = fs::File::create(path).await?;
    file.write_all(format!("P5\n{} {}\n255\n", frame.meta.width, frame.meta.height).as_bytes())
        .await?;
    file.write_all(&frame.bytes).await?;
    Ok(())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn io_error(error: std::io::Error) -> ApiError {
    ApiError::Failed(error.to_string())
}
