//! Public component contracts for camera, vision processing, and recording.

use async_trait::async_trait;
use bytes::Bytes;
use comm_core::{
    ApiError, CommandId, CommandReceipt, CorrelationId, EventStream, Versioned, system_time_serde,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::SystemTime};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    Gray8,
    Rgb8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointF32 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectF32 {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectF32 {
    pub fn clamp_to_image(self, width: u32, height: u32) -> Option<Self> {
        let x0 = self.x.max(0.0).min(width as f32);
        let y0 = self.y.max(0.0).min(height as f32);
        let x1 = (self.x + self.width).max(0.0).min(width as f32);
        let y1 = (self.y + self.height).max(0.0).min(height as f32);
        let clamped = Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        };
        (clamped.width >= 2.0 && clamped.height >= 2.0).then_some(clamped)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMeta {
    pub frame_id: u64,
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: PixelFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub meta: FrameMeta,
    pub bytes: Bytes,
}

pub type FrameStream = watch::Receiver<Option<Arc<Frame>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlgorithmId {
    TemplateNcc,
    EdgeModelMatch,
    RadialSymmetry,
    RingGridTarget,
    ChessCorners,
    CalibrationTarget,
}

/// Physical layout for the v1 coded-hex RingGrid calibration target.
///
/// All lengths are measured in millimeters. Marker centers and all detection
/// output remain in image-pixel coordinates; this configuration is only the
/// board-side description supplied to the detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RingGridTargetConfig {
    pub rows: u16,
    pub long_row_cols: u16,
    pub pitch_mm: f32,
    pub outer_radius_mm: f32,
    pub inner_radius_mm: f32,
    pub ring_width_mm: f32,
}

impl Default for RingGridTargetConfig {
    fn default() -> Self {
        Self {
            rows: 15,
            long_row_cols: 14,
            pitch_mm: 8.0,
            outer_radius_mm: 4.8,
            inner_radius_mm: 3.2,
            ring_width_mm: 1.152,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub frame_id: u64,
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,
    pub object_id: String,
    pub confidence: f32,
    pub bbox: Option<RectF32>,
    pub points: Vec<PointF32>,
    pub method: AlgorithmId,
    pub latency_us: u64,
    pub diagnostics: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraLifecycle {
    Disconnected,
    Connecting,
    Ready,
    Streaming,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraFormatInfo {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub min_fps: f32,
    pub max_fps: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraPermissionStatus {
    Unknown,
    NotDetermined,
    Authorized,
    Denied,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraPosition {
    Unknown,
    Front,
    Back,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraTransport {
    Unknown,
    BuiltIn,
    Continuity,
    Usb,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraDeviceInfo {
    pub id: String,
    pub display_name: String,
    pub model_id: Option<String>,
    pub manufacturer: Option<String>,
    pub position: CameraPosition,
    pub transport: CameraTransport,
    pub is_default: bool,
    pub formats: Vec<CameraFormatInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraState {
    pub lifecycle: CameraLifecycle,
    pub available_devices: Vec<CameraDeviceInfo>,
    pub active_device_id: Option<String>,
    pub active_format_id: Option<String>,
    pub permission_status: CameraPermissionStatus,
    pub requested_fps: f32,
    pub actual_fps: f32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub frame_id: u64,
    pub dropped_frames: u64,
    pub error: Option<String>,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            lifecycle: CameraLifecycle::Disconnected,
            available_devices: Vec::new(),
            active_device_id: None,
            active_format_id: None,
            permission_status: CameraPermissionStatus::Unknown,
            requested_fps: 30.0,
            actual_fps: 0.0,
            frame_width: 0,
            frame_height: 0,
            frame_id: 0,
            dropped_frames: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraCommand {
    pub command_id: CommandId,
    pub correlation_id: Option<CorrelationId>,
    pub kind: CameraCommandKind,
}

impl CameraCommand {
    pub fn new(kind: CameraCommandKind) -> Self {
        Self {
            command_id: CommandId::new(),
            correlation_id: None,
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CameraCommandKind {
    RefreshDevices,
    Connect,
    SelectDevice { device_id: String },
    SelectFormat { format_id: String },
    StartStream,
    StopStream,
    SetRequestedFps { fps: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CameraEvent {
    LifecycleChanged {
        lifecycle: CameraLifecycle,
    },
    DevicesChanged {
        count: usize,
    },
    ActiveConfigChanged {
        device_id: Option<String>,
        format_id: Option<String>,
    },
    RequestedFpsChanged {
        fps: f32,
    },
    FrameProduced {
        frame_id: u64,
    },
    DroppedFramesChanged {
        dropped_frames: u64,
    },
    Error {
        message: String,
    },
}

pub type CameraEventStream = EventStream<CameraEvent>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionLifecycle {
    Idle,
    WaitingForTemplate,
    Processing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionState {
    pub lifecycle: VisionLifecycle,
    pub selected_algorithm: AlgorithmId,
    pub ringgrid_target: RingGridTargetConfig,
    pub roi: Option<RectF32>,
    pub has_template: bool,
    pub input_fps: f32,
    pub processing_fps: f32,
    pub mean_latency_ms: f32,
    pub dropped_input_frames: u64,
    pub last_detection: Option<Detection>,
    pub error: Option<String>,
}

impl Default for VisionState {
    fn default() -> Self {
        Self {
            lifecycle: VisionLifecycle::Idle,
            selected_algorithm: AlgorithmId::ChessCorners,
            ringgrid_target: RingGridTargetConfig::default(),
            roi: None,
            has_template: false,
            input_fps: 0.0,
            processing_fps: 0.0,
            mean_latency_ms: 0.0,
            dropped_input_frames: 0,
            last_detection: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionCommand {
    pub command_id: CommandId,
    pub correlation_id: Option<CorrelationId>,
    pub kind: VisionCommandKind,
}

impl VisionCommand {
    pub fn new(kind: VisionCommandKind) -> Self {
        Self {
            command_id: CommandId::new(),
            correlation_id: None,
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisionCommandKind {
    SelectAlgorithm {
        algorithm: AlgorithmId,
    },
    SetRingGridTargetConfig {
        config: RingGridTargetConfig,
    },
    SetRoi {
        roi: Option<RectF32>,
    },
    CaptureTemplate,
    StartProcessing,
    StopProcessing,
    #[cfg(feature = "dev-faults")]
    InjectFault(VisionFault),
}

#[cfg(feature = "dev-faults")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisionFault {
    DelayProcessingMs(u64),
    DropEveryNthEvent(u64),
    RestartRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisionEvent {
    LifecycleChanged { lifecycle: VisionLifecycle },
    AlgorithmSelected { algorithm: AlgorithmId },
    RingGridTargetConfigChanged { config: RingGridTargetConfig },
    RoiChanged { roi: Option<RectF32> },
    TemplateCaptured { width: u32, height: u32 },
    DetectionProduced { detection: Detection },
    MetricsUpdated,
    Error { message: String },
}

pub type VisionEventStream = EventStream<VisionEvent>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecorderLifecycle {
    Idle,
    Recording,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderState {
    pub lifecycle: RecorderLifecycle,
    pub session_path: Option<String>,
    pub recorded_frames: u64,
    pub recorded_detections: u64,
    pub dropped_frames: u64,
    pub error: Option<String>,
}

/// Summary of a persisted recording session owned by the recorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    /// Stable directory name relative to the recorder's configured session root.
    pub id: String,
    pub created_at_ms: u64,
    pub frame_count: u64,
    pub detection_count: u64,
}

/// Metadata for a single frame available from a persisted recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedFrame {
    pub meta: FrameMeta,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self {
            lifecycle: RecorderLifecycle::Idle,
            session_path: None,
            recorded_frames: 0,
            recorded_detections: 0,
            dropped_frames: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderCommand {
    pub command_id: CommandId,
    pub correlation_id: Option<CorrelationId>,
    pub kind: RecorderCommandKind,
}

impl RecorderCommand {
    pub fn new(kind: RecorderCommandKind) -> Self {
        Self {
            command_id: CommandId::new(),
            correlation_id: None,
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderCommandKind {
    StartRecording { max_fps: f32 },
    StopRecording,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderEvent {
    LifecycleChanged { lifecycle: RecorderLifecycle },
    SessionStarted { path: String },
    SessionStopped { path: String },
    FrameRecorded { frame_id: u64 },
    DetectionRecorded { frame_id: u64 },
    Error { message: String },
}

pub type RecorderEventStream = EventStream<RecorderEvent>;

#[async_trait]
pub trait CameraApi: Send + Sync {
    async fn submit(&self, command: CameraCommand) -> Result<CommandReceipt, ApiError>;
    async fn get_state(&self) -> Result<Versioned<CameraState>, ApiError>;
    async fn subscribe(&self) -> Result<CameraEventStream, ApiError>;
    async fn subscribe_frames(&self) -> Result<FrameStream, ApiError>;
}

#[async_trait]
pub trait VisionApi: Send + Sync {
    async fn submit(&self, command: VisionCommand) -> Result<CommandReceipt, ApiError>;
    async fn get_state(&self) -> Result<Versioned<VisionState>, ApiError>;
    async fn subscribe(&self) -> Result<VisionEventStream, ApiError>;
}

#[async_trait]
pub trait RecorderApi: Send + Sync {
    async fn submit(&self, command: RecorderCommand) -> Result<CommandReceipt, ApiError>;
    async fn get_state(&self) -> Result<Versioned<RecorderState>, ApiError>;
    async fn subscribe(&self) -> Result<RecorderEventStream, ApiError>;
    async fn list_sessions(&self) -> Result<Vec<RecordedSession>, ApiError>;
    async fn list_session_frames(&self, session_id: &str) -> Result<Vec<RecordedFrame>, ApiError>;
    async fn read_session_frame(&self, session_id: &str, frame_id: u64) -> Result<Frame, ApiError>;
}
