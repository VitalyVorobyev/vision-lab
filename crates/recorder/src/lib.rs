//! Recorder component runtime.

use async_trait::async_trait;
use comm_core::{
    ApiError, CommandReceipt, ComponentIdentity, OperationId, Versioned, event, now, versioned,
};
use comm_local::{
    CommandClient, CommandInbox, EventBus, MonotonicCounter, StateCell, command_channel,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{fs, sync::watch};
use vision_contracts::{
    CameraApi, Frame, FrameMeta, PixelFormat, RecordedFrame, RecordedSession, RecorderApi,
    RecorderCommand, RecorderCommandKind, RecorderEvent, RecorderEventStream, RecorderLifecycle,
    RecorderState, VisionApi, VisionEvent,
};

#[derive(Clone)]
pub struct RecorderComponent {
    client: CommandClient<RecorderCommand, CommandReceipt>,
    state: Arc<StateCell<Versioned<RecorderState>>>,
    events: Arc<EventBus<RecorderEvent>>,
    base_dir: PathBuf,
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
            base_dir: base_dir.clone(),
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

    async fn list_sessions(&self) -> Result<Vec<RecordedSession>, ApiError> {
        list_recorded_sessions(&self.base_dir).await
    }

    async fn list_session_frames(&self, session_id: &str) -> Result<Vec<RecordedFrame>, ApiError> {
        let session_dir = resolve_session_dir(&self.base_dir, session_id)?;
        read_frame_records(&session_dir)
            .await?
            .into_iter()
            .map(FrameRecord::recorded_frame)
            .collect()
    }

    async fn read_session_frame(&self, session_id: &str, frame_id: u64) -> Result<Frame, ApiError> {
        let session_dir = resolve_session_dir(&self.base_dir, session_id)?;
        let record = read_frame_records(&session_dir)
            .await?
            .into_iter()
            .find(|record| record.frame_id == frame_id)
            .ok_or_else(|| ApiError::InvalidRequest("recorded frame was not found".into()))?;
        let frame_path = session_dir
            .join("frames")
            .join(format!("frame_{:08}.pgm", record.frame_id));
        let bytes = read_pgm(&frame_path, record.width, record.height).await?;
        Ok(Frame {
            meta: record.meta(),
            bytes: bytes.into(),
        })
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
            timestamp_ms: frame
                .meta
                .timestamp
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            width: frame.meta.width,
            height: frame.meta.height,
            stride: frame.meta.width,
            pixel_format: Some(PixelFormat::Gray8),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrameRecord {
    frame_id: u64,
    #[serde(default)]
    timestamp_ms: u64,
    width: u32,
    height: u32,
    #[serde(default)]
    stride: u32,
    #[serde(default)]
    pixel_format: Option<PixelFormat>,
    path: String,
}

impl FrameRecord {
    fn meta(self) -> FrameMeta {
        FrameMeta {
            frame_id: self.frame_id,
            timestamp: UNIX_EPOCH + Duration::from_millis(self.timestamp_ms),
            width: self.width,
            height: self.height,
            stride: self.stride.max(self.width),
            pixel_format: self.pixel_format.unwrap_or(PixelFormat::Gray8),
        }
    }

    fn recorded_frame(self) -> Result<RecordedFrame, ApiError> {
        Ok(RecordedFrame { meta: self.meta() })
    }
}

async fn list_recorded_sessions(base_dir: &Path) -> Result<Vec<RecordedSession>, ApiError> {
    let mut entries = match fs::read_dir(base_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    let mut sessions = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(io_error)? {
        if !entry.file_type().await.map_err(io_error)?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if !is_session_id(&id) {
            continue;
        }
        let session_dir = entry.path();
        let frame_count = read_frame_records(&session_dir)
            .await
            .map(|records| records.len() as u64)
            .unwrap_or_default();
        let detection_count = count_jsonl_records(&session_dir.join("detections.jsonl")).await;
        let created_at_ms = id
            .strip_prefix("session-")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default();
        sessions.push(RecordedSession {
            id,
            created_at_ms,
            frame_count,
            detection_count,
        });
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(session.created_at_ms));
    Ok(sessions)
}

fn resolve_session_dir(base_dir: &Path, session_id: &str) -> Result<PathBuf, ApiError> {
    if !is_session_id(session_id) {
        return Err(ApiError::InvalidRequest(
            "invalid recorded session id".into(),
        ));
    }
    Ok(base_dir.join(session_id))
}

fn is_session_id(value: &str) -> bool {
    value.strip_prefix("session-").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

async fn read_frame_records(session_dir: &Path) -> Result<Vec<FrameRecord>, ApiError> {
    let contents = fs::read_to_string(session_dir.join("frames.jsonl"))
        .await
        .map_err(io_error)?;
    let mut records: Vec<FrameRecord> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|error| ApiError::Failed(error.to_string())))
        .collect::<Result<_, _>>()?;
    records.sort_by_key(|record| record.frame_id);
    Ok(records)
}

async fn count_jsonl_records(path: &Path) -> u64 {
    fs::read_to_string(path)
        .await
        .map(|contents| {
            contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count() as u64
        })
        .unwrap_or_default()
}

async fn read_pgm(
    path: &Path,
    expected_width: u32,
    expected_height: u32,
) -> Result<Vec<u8>, ApiError> {
    let data = fs::read(path).await.map_err(io_error)?;
    let mut offset = 0;
    let magic = next_pgm_token(&data, &mut offset)
        .ok_or_else(|| ApiError::Failed("recorded frame has no PGM header".into()))?;
    if magic != b"P5" {
        return Err(ApiError::Failed("recorded frame is not binary PGM".into()));
    }
    let width = parse_pgm_u32(next_pgm_token(&data, &mut offset), "width")?;
    let height = parse_pgm_u32(next_pgm_token(&data, &mut offset), "height")?;
    let max_value = parse_pgm_u32(next_pgm_token(&data, &mut offset), "max value")?;
    if data
        .get(offset)
        .is_none_or(|byte| !byte.is_ascii_whitespace())
    {
        return Err(ApiError::Failed(
            "recorded PGM has no pixel separator".into(),
        ));
    }
    offset += 1;
    if width != expected_width || height != expected_height || max_value != 255 {
        return Err(ApiError::Failed(
            "recorded PGM metadata does not match its frame record".into(),
        ));
    }
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| ApiError::Failed("recorded PGM dimensions are too large".into()))?;
    let bytes = data
        .get(offset..)
        .ok_or_else(|| ApiError::Failed("recorded PGM has no pixels".into()))?;
    if bytes.len() != pixels {
        return Err(ApiError::Failed(
            "recorded PGM pixel data has an invalid length".into(),
        ));
    }
    Ok(bytes.to_vec())
}

fn next_pgm_token<'a>(data: &'a [u8], offset: &mut usize) -> Option<&'a [u8]> {
    while *offset < data.len() {
        if data[*offset].is_ascii_whitespace() {
            *offset += 1;
        } else if data[*offset] == b'#' {
            while *offset < data.len() && data[*offset] != b'\n' {
                *offset += 1;
            }
        } else {
            break;
        }
    }
    let start = *offset;
    while *offset < data.len() && !data[*offset].is_ascii_whitespace() {
        *offset += 1;
    }
    (start < *offset).then_some(&data[start..*offset])
}

fn parse_pgm_u32(value: Option<&[u8]>, label: &str) -> Result<u32, ApiError> {
    let value = value.ok_or_else(|| ApiError::Failed(format!("recorded PGM has no {label}")))?;
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| ApiError::Failed(format!("recorded PGM has an invalid {label}")))
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

    let bytes = frame_as_gray(frame)?;
    let mut file = fs::File::create(path).await?;
    file.write_all(format!("P5\n{} {}\n255\n", frame.meta.width, frame.meta.height).as_bytes())
        .await?;
    file.write_all(&bytes).await?;
    Ok(())
}

fn frame_as_gray(frame: &Frame) -> Result<Vec<u8>, std::io::Error> {
    let width = frame.meta.width as usize;
    let height = frame.meta.height as usize;
    let stride = frame.meta.stride as usize;
    let row_bytes = match frame.meta.pixel_format {
        PixelFormat::Gray8 => width,
        PixelFormat::Rgb8 => width * 3,
    };
    if stride < row_bytes || frame.bytes.len() < stride.saturating_mul(height) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame data does not match its declared layout",
        ));
    }
    let mut gray = Vec::with_capacity(width * height);
    for row_index in 0..height {
        let row_start = row_index * stride;
        let row = &frame.bytes[row_start..row_start + row_bytes];
        match frame.meta.pixel_format {
            PixelFormat::Gray8 => gray.extend_from_slice(row),
            PixelFormat::Rgb8 => {
                for rgb in row.chunks_exact(3) {
                    let luma = (77 * u16::from(rgb[0])
                        + 150 * u16::from(rgb[1])
                        + 29 * u16::from(rgb[2])
                        + 128)
                        >> 8;
                    gray.push(luma as u8);
                }
            }
        }
    }
    Ok(gray)
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
