//! Vision processing component runtime.

use async_trait::async_trait;
use comm_core::{
    ApiError, CommandReceipt, ComponentIdentity, CorrelationId, OperationId, Versioned, event,
    versioned,
};
use comm_local::{
    CommandClient, CommandInbox, EventBus, MonotonicCounter, StateCell, command_channel,
};
use image::GrayImage;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use vision_contracts::{
    AlgorithmId, CameraApi, Detection, Frame, PixelFormat, PointF32, RectF32, RingGridTargetConfig,
    VisionApi, VisionCommand, VisionCommandKind, VisionEvent, VisionEventStream, VisionLifecycle,
    VisionState,
};

#[derive(Clone)]
pub struct VisionComponent {
    client: CommandClient<VisionCommand, CommandReceipt>,
    state: Arc<StateCell<Versioned<VisionState>>>,
    events: Arc<EventBus<VisionEvent>>,
}

impl VisionComponent {
    pub async fn spawn(
        component_name: &str,
        camera: Arc<dyn CameraApi>,
    ) -> Result<Arc<Self>, ApiError> {
        let identity = ComponentIdentity::new("vision", component_name, env!("CARGO_PKG_VERSION"));
        let initial = versioned(identity.clone(), 0, VisionState::default());
        let state = Arc::new(StateCell::new(initial));
        let events = Arc::new(EventBus::new(512));
        let (client, inbox) = command_channel(32);
        let frames = camera.subscribe_frames().await?;
        let component = Arc::new(Self {
            client,
            state: state.clone(),
            events: events.clone(),
        });
        tokio::spawn(run_vision(identity, inbox, state, events, frames));
        Ok(component)
    }
}

#[async_trait]
impl VisionApi for VisionComponent {
    async fn submit(&self, command: VisionCommand) -> Result<CommandReceipt, ApiError> {
        self.client.submit(command).await
    }

    async fn get_state(&self) -> Result<Versioned<VisionState>, ApiError> {
        Ok(self.state.get().await)
    }

    async fn subscribe(&self) -> Result<VisionEventStream, ApiError> {
        Ok(self.events.subscribe())
    }
}

struct Runtime {
    identity: ComponentIdentity,
    state_cell: Arc<StateCell<Versioned<VisionState>>>,
    events: Arc<EventBus<VisionEvent>>,
    sequence: MonotonicCounter,
    revision: MonotonicCounter,
    state: VisionState,
    latest_frame: Option<Arc<Frame>>,
    detector: Option<ActiveDetector>,
    processed_frames: u64,
    last_processed_frame_id: u64,
    started_at: Instant,
}

async fn run_vision(
    identity: ComponentIdentity,
    mut inbox: CommandInbox<VisionCommand, CommandReceipt>,
    state_cell: Arc<StateCell<Versioned<VisionState>>>,
    events: Arc<EventBus<VisionEvent>>,
    mut frames: watch::Receiver<Option<Arc<Frame>>>,
) {
    let mut runtime = Runtime {
        identity,
        state_cell,
        events,
        sequence: MonotonicCounter::default(),
        revision: MonotonicCounter::default(),
        state: VisionState::default(),
        latest_frame: None,
        detector: None,
        processed_frames: 0,
        last_processed_frame_id: 0,
        started_at: Instant::now(),
    };

    loop {
        tokio::select! {
            changed = frames.changed() => {
                if changed.is_err() {
                    runtime.set_error("camera frame stream closed").await;
                    break;
                }
                let frame = frames.borrow().clone();
                runtime.latest_frame = frame;
                runtime.process_latest_frame().await;
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
        command: &VisionCommand,
    ) -> Result<CommandReceipt, ApiError> {
        match &command.kind {
            VisionCommandKind::SelectAlgorithm { algorithm } => {
                self.select_algorithm(*algorithm, command.correlation_id);
            }
            VisionCommandKind::SetRingGridTargetConfig { config } => {
                self.set_ringgrid_target_config(config, command.correlation_id)?;
            }
            VisionCommandKind::SetRoi { roi } => {
                self.set_roi(*roi, command.correlation_id);
            }
            VisionCommandKind::CaptureTemplate => {
                self.capture_template(command.correlation_id)?;
            }
            VisionCommandKind::StartProcessing => {
                self.start_processing(command.correlation_id).await?;
            }
            VisionCommandKind::StopProcessing => {
                self.stop_processing(command.correlation_id);
            }
        }

        let accepted_revision = self.store().await;
        Ok(CommandReceipt {
            command_id: command.command_id,
            operation_id: Some(OperationId::new()),
            accepted_revision,
        })
    }

    fn select_algorithm(&mut self, algorithm: AlgorithmId, correlation_id: Option<CorrelationId>) {
        self.state.selected_algorithm = algorithm;
        self.detector = None;
        self.state.has_template = false;
        self.state.lifecycle = lifecycle_for_algorithm(algorithm);
        self.publish(correlation_id, VisionEvent::AlgorithmSelected { algorithm });
    }

    fn set_roi(&mut self, roi: Option<RectF32>, correlation_id: Option<CorrelationId>) {
        self.state.roi = roi;
        self.detector = None;
        self.state.has_template = false;
        self.state.lifecycle = lifecycle_for_algorithm(self.state.selected_algorithm);
        self.publish(correlation_id, VisionEvent::RoiChanged { roi });
    }

    fn set_ringgrid_target_config(
        &mut self,
        config: &RingGridTargetConfig,
        correlation_id: Option<CorrelationId>,
    ) -> Result<(), ApiError> {
        if self.state.lifecycle == VisionLifecycle::Processing {
            return Err(ApiError::Rejected(
                "stop processing before changing the RingGrid target configuration".into(),
            ));
        }
        RingGridDetector::new(config)?;
        self.state.ringgrid_target = config.clone();
        self.detector = None;
        self.state.error = None;
        self.publish(
            correlation_id,
            VisionEvent::RingGridTargetConfigChanged {
                config: config.clone(),
            },
        );
        Ok(())
    }

    fn capture_template(&mut self, correlation_id: Option<CorrelationId>) -> Result<(), ApiError> {
        if self.state.selected_algorithm != AlgorithmId::TemplateNcc {
            return Err(ApiError::Rejected(
                "template capture is only implemented for TemplateNcc in v1".into(),
            ));
        }
        let frame = self
            .latest_frame
            .as_ref()
            .ok_or_else(|| ApiError::Rejected("no camera frame available".into()))?;
        let roi = self
            .state
            .roi
            .ok_or_else(|| ApiError::Rejected("set an ROI before capturing a template".into()))?;
        let detector = TemplateNcc::capture(frame, roi)?;
        let width = detector.width;
        let height = detector.height;
        self.detector = Some(ActiveDetector::TemplateNcc(detector));
        self.state.has_template = true;
        self.state.lifecycle = VisionLifecycle::Idle;
        self.state.error = None;
        self.publish(
            correlation_id,
            VisionEvent::TemplateCaptured { width, height },
        );
        Ok(())
    }

    async fn start_processing(
        &mut self,
        correlation_id: Option<CorrelationId>,
    ) -> Result<(), ApiError> {
        if self.needs_template_before_processing() {
            self.state.lifecycle = VisionLifecycle::WaitingForTemplate;
            self.store().await;
            return Err(ApiError::Rejected(
                "capture a template before starting processing".into(),
            ));
        }
        if self.detector.is_none() {
            self.detector = Some(ActiveDetector::for_algorithm(
                self.state.selected_algorithm,
                &self.state.ringgrid_target,
            )?);
        }
        self.state.lifecycle = VisionLifecycle::Processing;
        self.state.error = None;
        self.started_at = Instant::now();
        self.processed_frames = 0;
        self.publish_lifecycle(correlation_id);
        Ok(())
    }

    fn stop_processing(&mut self, correlation_id: Option<CorrelationId>) {
        self.state.lifecycle = VisionLifecycle::Idle;
        self.publish_lifecycle(correlation_id);
    }

    fn needs_template_before_processing(&self) -> bool {
        self.state.selected_algorithm == AlgorithmId::TemplateNcc
            && !matches!(self.detector, Some(ActiveDetector::TemplateNcc(_)))
    }

    fn publish_lifecycle(&self, correlation_id: Option<CorrelationId>) {
        self.publish(
            correlation_id,
            VisionEvent::LifecycleChanged {
                lifecycle: self.state.lifecycle,
            },
        );
    }

    async fn process_latest_frame(&mut self) {
        if self.state.lifecycle != VisionLifecycle::Processing {
            return;
        }
        let Some(frame) = self.latest_frame.clone() else {
            return;
        };
        if frame.meta.frame_id == self.last_processed_frame_id {
            return;
        }
        if frame.meta.frame_id > self.last_processed_frame_id + 1
            && self.last_processed_frame_id != 0
        {
            self.state.dropped_input_frames +=
                frame.meta.frame_id - self.last_processed_frame_id - 1;
        }
        self.last_processed_frame_id = frame.meta.frame_id;

        let Some(detector) = self.detector.as_mut() else {
            self.state.lifecycle = VisionLifecycle::WaitingForTemplate;
            self.store().await;
            return;
        };
        let started = Instant::now();
        match detector.detect(&frame) {
            Ok(Some(mut detection)) => {
                detection.latency_us = started.elapsed().as_micros() as u64;
                self.processed_frames = self.processed_frames.saturating_add(1);
                self.state.processing_fps = rate(self.processed_frames, self.started_at.elapsed());
                self.state.input_fps = self.state.processing_fps;
                let latency_ms = detection.latency_us as f32 / 1000.0;
                self.state.mean_latency_ms = if self.processed_frames == 1 {
                    latency_ms
                } else {
                    self.state.mean_latency_ms * 0.9 + latency_ms * 0.1
                };
                self.state.last_detection = Some(detection.clone());
                self.publish(None, VisionEvent::DetectionProduced { detection });
                self.store().await;
            }
            Ok(None) => {
                self.processed_frames = self.processed_frames.saturating_add(1);
                self.state.processing_fps = rate(self.processed_frames, self.started_at.elapsed());
                self.store().await;
            }
            Err(error) => {
                self.set_error(&error).await;
            }
        }
    }

    async fn set_error(&mut self, message: &str) {
        self.state.lifecycle = VisionLifecycle::Error;
        self.state.error = Some(message.to_string());
        self.publish(
            None,
            VisionEvent::Error {
                message: message.to_string(),
            },
        );
        self.store().await;
    }

    fn publish(&self, correlation_id: Option<comm_core::CorrelationId>, payload: VisionEvent) {
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

fn rate(count: u64, elapsed: Duration) -> f32 {
    let secs = elapsed.as_secs_f32();
    if secs > 0.0 { count as f32 / secs } else { 0.0 }
}

enum ActiveDetector {
    TemplateNcc(TemplateNcc),
    RadialSymmetry,
    ChessCorners(Box<chess_corners::Detector>),
    CalibrationTarget,
    RingGrid(Box<RingGridDetector>),
}

impl ActiveDetector {
    fn for_algorithm(
        algorithm: AlgorithmId,
        ringgrid_target: &RingGridTargetConfig,
    ) -> Result<Self, ApiError> {
        match algorithm {
            AlgorithmId::TemplateNcc => Err(ApiError::Rejected(
                "capture a template before starting TemplateNcc".into(),
            )),
            AlgorithmId::RadialSymmetry => Ok(Self::RadialSymmetry),
            AlgorithmId::ChessCorners => {
                let detector = chess_corners::Detector::new(chess_corners::DetectorConfig::chess())
                    .map_err(|error| ApiError::Failed(error.to_string()))?;
                Ok(Self::ChessCorners(Box::new(detector)))
            }
            AlgorithmId::CalibrationTarget => Ok(Self::CalibrationTarget),
            AlgorithmId::RingGridTarget => Ok(Self::RingGrid(Box::new(RingGridDetector::new(
                ringgrid_target,
            )?))),
            AlgorithmId::EdgeModelMatch => Err(ApiError::Rejected(
                "EdgeModelMatch is deferred until vision-metrology is published".into(),
            )),
        }
    }

    fn detect(&mut self, frame: &Frame) -> Result<Option<Detection>, String> {
        match self {
            Self::TemplateNcc(detector) => detector.detect(frame),
            Self::RadialSymmetry => detect_radial_symmetry(frame),
            Self::ChessCorners(detector) => detect_chess_corners(detector.as_mut(), frame),
            Self::CalibrationTarget => detect_calibration_target(frame),
            Self::RingGrid(detector) => detector.detect(frame),
        }
    }
}

struct RingGridDetector {
    detector: ringgrid::Detector,
    expected_markers: usize,
}

impl RingGridDetector {
    fn new(config: &RingGridTargetConfig) -> Result<Self, ApiError> {
        let target = ringgrid::TargetLayout::coded_hex(
            config.pitch_mm,
            usize::from(config.rows),
            usize::from(config.long_row_cols),
            config.outer_radius_mm,
            config.inner_radius_mm,
            config.ring_width_mm,
        )
        .map_err(|error| ApiError::InvalidRequest(format!("invalid RingGrid target: {error}")))?;
        let expected_markers = target.cells().len();
        Ok(Self {
            detector: ringgrid::Detector::new(target),
            expected_markers,
        })
    }

    fn detect(&mut self, frame: &Frame) -> Result<Option<Detection>, String> {
        let image = frame_to_gray_image(frame)?;
        let result = self
            .detector
            .detect(&image)
            .map_err(|error| error.to_string())?;
        if result.center_frame != ringgrid::DetectionFrame::Image {
            return Err("RingGrid returned marker centers outside the image frame".into());
        }
        if result.detected_markers.is_empty() {
            return Ok(None);
        }
        let points: Result<Vec<PointF32>, String> = result
            .detected_markers
            .iter()
            .map(|marker| image_point(marker.center))
            .collect();
        let points = points?;
        let marker_count = points.len();
        Ok(Some(Detection {
            frame_id: frame.meta.frame_id,
            timestamp: frame.meta.timestamp,
            object_id: "ringgrid-target".into(),
            confidence: (marker_count as f32 / self.expected_markers as f32).clamp(0.0, 1.0),
            bbox: bbox_from_points(&points),
            points,
            method: AlgorithmId::RingGridTarget,
            latency_us: 0,
            diagnostics: Some(format!(
                "markers={marker_count}/{} centers=image-px",
                self.expected_markers
            )),
        }))
    }
}

fn image_point(center: [f64; 2]) -> Result<PointF32, String> {
    let x = checked_f32(center[0])?;
    let y = checked_f32(center[1])?;
    Ok(PointF32 { x, y })
}

fn checked_f32(value: f64) -> Result<f32, String> {
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err("RingGrid returned a non-finite or out-of-range image coordinate".into());
    }
    Ok(value as f32)
}

fn frame_to_gray_image(frame: &Frame) -> Result<GrayImage, String> {
    let width = usize::try_from(frame.meta.width).map_err(|_| "frame width is too large")?;
    let height = usize::try_from(frame.meta.height).map_err(|_| "frame height is too large")?;
    let stride = usize::try_from(frame.meta.stride).map_err(|_| "frame stride is too large")?;
    let row_bytes = match frame.meta.pixel_format {
        PixelFormat::Gray8 => width,
        PixelFormat::Rgb8 => width
            .checked_mul(3)
            .ok_or_else(|| "RGB frame row is too wide".to_string())?,
    };
    if stride < row_bytes {
        return Err("frame stride is smaller than its pixel row".into());
    }
    let required = stride
        .checked_mul(height)
        .ok_or_else(|| "frame is too large".to_string())?;
    if frame.bytes.len() < required {
        return Err("frame data is shorter than its declared stride".into());
    }

    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| "frame is too large".to_string())?;
    let mut gray = Vec::with_capacity(pixels);
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
    GrayImage::from_raw(frame.meta.width, frame.meta.height, gray)
        .ok_or_else(|| "failed to construct grayscale image".into())
}

fn lifecycle_for_algorithm(algorithm: AlgorithmId) -> VisionLifecycle {
    if algorithm == AlgorithmId::TemplateNcc {
        VisionLifecycle::WaitingForTemplate
    } else {
        VisionLifecycle::Idle
    }
}

#[derive(Debug, Clone)]
struct TemplateNcc {
    width: u32,
    height: u32,
    data: Vec<u8>,
    mean: f32,
    norm: f32,
}

fn detect_radial_symmetry(frame: &Frame) -> Result<Option<Detection>, String> {
    let view = radsym::ImageView::new(
        &frame.bytes,
        frame.meta.width as usize,
        frame.meta.height as usize,
        frame.meta.stride as usize,
    )
    .map_err(|error| error.to_string())?;
    let base_radius = (frame.meta.width.min(frame.meta.height) / 18).clamp(4, 24);
    let radii = [
        base_radius.saturating_sub(2).max(2),
        base_radius,
        base_radius + 2,
    ];
    let config = radsym::DetectCirclesConfig::for_radii(radii)
        .polarity(radsym::Polarity::Both)
        .radius_hint(base_radius as f32)
        .min_score(0.2);
    let detections = radsym::detect_circles(&view, &config).map_err(|error| error.to_string())?;
    let Some(best) = detections.first() else {
        return Ok(None);
    };
    let circle = best.hypothesis;
    let radius = circle.radius.max(1.0);
    Ok(Some(Detection {
        frame_id: frame.meta.frame_id,
        timestamp: frame.meta.timestamp,
        object_id: "radial-symmetry-circle".into(),
        confidence: best.score.total.clamp(0.0, 1.0),
        bbox: Some(RectF32 {
            x: circle.center.x - radius,
            y: circle.center.y - radius,
            width: radius * 2.0,
            height: radius * 2.0,
        }),
        points: vec![PointF32 {
            x: circle.center.x,
            y: circle.center.y,
        }],
        method: AlgorithmId::RadialSymmetry,
        latency_us: 0,
        diagnostics: Some(format!("radius={:.2}", circle.radius)),
    }))
}

fn detect_chess_corners(
    detector: &mut chess_corners::Detector,
    frame: &Frame,
) -> Result<Option<Detection>, String> {
    let corners = detector
        .detect_u8(&frame.bytes, frame.meta.width, frame.meta.height)
        .map_err(|error| error.to_string())?;
    if corners.is_empty() {
        return Ok(None);
    }
    let points: Vec<PointF32> = corners
        .iter()
        .map(|corner| PointF32 {
            x: corner.x,
            y: corner.y,
        })
        .collect();
    let bbox = bbox_from_points(&points);
    let max_response = corners
        .iter()
        .map(|corner| corner.response)
        .fold(0.0_f32, f32::max);
    Ok(Some(Detection {
        frame_id: frame.meta.frame_id,
        timestamp: frame.meta.timestamp,
        object_id: "chess-corners".into(),
        confidence: response_confidence(max_response),
        bbox,
        points,
        method: AlgorithmId::ChessCorners,
        latency_us: 0,
        diagnostics: Some(format!("corners={}", corners.len())),
    }))
}

fn detect_calibration_target(frame: &Frame) -> Result<Option<Detection>, String> {
    let chess_cfg = calib_targets::detect::default_chess_config();
    let params = calib_targets::chessboard::DetectorParams::default();
    let detection = calib_targets::detect::detect_chessboard_from_gray_u8(
        frame.meta.width,
        frame.meta.height,
        &frame.bytes,
        &chess_cfg,
        &params,
    )
    .map_err(|error| error.to_string())?;
    let Some(detection) = detection else {
        return Ok(None);
    };
    let points: Vec<PointF32> = detection
        .corners
        .iter()
        .map(|corner| PointF32 {
            x: corner.position.x,
            y: corner.position.y,
        })
        .collect();
    Ok(Some(Detection {
        frame_id: frame.meta.frame_id,
        timestamp: frame.meta.timestamp,
        object_id: "calibration-chessboard".into(),
        confidence: (detection.corners.len() as f32 / 64.0).clamp(0.0, 1.0),
        bbox: bbox_from_points(&points),
        points,
        method: AlgorithmId::CalibrationTarget,
        latency_us: 0,
        diagnostics: Some(format!(
            "corners={} cell_size={:?}",
            detection.corners.len(),
            detection.cell_size
        )),
    }))
}

fn bbox_from_points(points: &[PointF32]) -> Option<RectF32> {
    let first = points.first()?;
    let (mut min_x, mut max_x) = (first.x, first.x);
    let (mut min_y, mut max_y) = (first.y, first.y);
    for point in points.iter().skip(1) {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Some(RectF32 {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1.0),
        height: (max_y - min_y).max(1.0),
    })
}

fn response_confidence(response: f32) -> f32 {
    if response <= 0.0 {
        0.0
    } else {
        (response / (response + 1000.0)).clamp(0.0, 1.0)
    }
}

impl TemplateNcc {
    fn capture(frame: &Frame, roi: RectF32) -> Result<Self, ApiError> {
        let roi = roi
            .clamp_to_image(frame.meta.width, frame.meta.height)
            .ok_or_else(|| ApiError::InvalidRequest("ROI is outside the frame".into()))?;
        let x0 = roi.x.round() as u32;
        let y0 = roi.y.round() as u32;
        let width = roi.width.round() as u32;
        let height = roi.height.round() as u32;
        let mut data = Vec::with_capacity((width * height) as usize);
        for y in y0..(y0 + height) {
            let row = &frame.bytes[(y * frame.meta.stride + x0) as usize
                ..(y * frame.meta.stride + x0 + width) as usize];
            data.extend_from_slice(row);
        }
        let mean = mean_u8(&data);
        let norm = norm_centered(&data, mean);
        if norm <= f32::EPSILON {
            return Err(ApiError::Rejected(
                "template has no contrast; choose a textured ROI".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            data,
            mean,
            norm,
        })
    }

    fn detect(&mut self, frame: &Frame) -> Result<Option<Detection>, String> {
        if frame.meta.width < self.width || frame.meta.height < self.height {
            return Ok(None);
        }
        let search_w = frame.meta.width - self.width;
        let search_h = frame.meta.height - self.height;
        let mut best_score = -1.0_f32;
        let mut best_x = 0_u32;
        let mut best_y = 0_u32;

        for y in 0..=search_h {
            for x in 0..=search_w {
                let score = self.score_at(frame, x, y);
                if score > best_score {
                    best_score = score;
                    best_x = x;
                    best_y = y;
                }
            }
        }

        (best_score >= 0.65)
            .then_some(Detection {
                frame_id: frame.meta.frame_id,
                timestamp: frame.meta.timestamp,
                object_id: "template".into(),
                confidence: best_score.clamp(0.0, 1.0),
                bbox: Some(RectF32 {
                    x: best_x as f32,
                    y: best_y as f32,
                    width: self.width as f32,
                    height: self.height as f32,
                }),
                points: Vec::new(),
                method: AlgorithmId::TemplateNcc,
                latency_us: 0,
                diagnostics: None,
            })
            .pipe(Ok)
    }

    fn score_at(&self, frame: &Frame, x0: u32, y0: u32) -> f32 {
        let mut sum = 0.0_f32;
        for y in 0..self.height {
            let base = ((y0 + y) * frame.meta.stride + x0) as usize;
            for x in 0..self.width {
                sum += frame.bytes[base + x as usize] as f32;
            }
        }
        let count = (self.width * self.height) as f32;
        let mean = sum / count;
        let mut numerator = 0.0_f32;
        let mut patch_norm = 0.0_f32;
        let mut i = 0_usize;
        for y in 0..self.height {
            let base = ((y0 + y) * frame.meta.stride + x0) as usize;
            for x in 0..self.width {
                let a = self.data[i] as f32 - self.mean;
                let b = frame.bytes[base + x as usize] as f32 - mean;
                numerator += a * b;
                patch_norm += b * b;
                i += 1;
            }
        }
        if patch_norm <= f32::EPSILON {
            return -1.0;
        }
        numerator / (self.norm * patch_norm.sqrt())
    }
}

fn mean_u8(data: &[u8]) -> f32 {
    data.iter().map(|value| *value as f32).sum::<f32>() / data.len() as f32
}

fn norm_centered(data: &[u8], mean: f32) -> f32 {
    data.iter()
        .map(|value| {
            let centered = *value as f32 - mean;
            centered * centered
        })
        .sum::<f32>()
        .sqrt()
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use comm_core::now;
    use vision_contracts::{FrameMeta, PixelFormat};

    #[test]
    fn ncc_finds_known_synthetic_target() {
        let template_frame = frame_with_square(1, 80, 60, 25, 25);
        let roi = RectF32 {
            x: 80.0,
            y: 60.0,
            width: 25.0,
            height: 25.0,
        };
        let mut detector = TemplateNcc::capture(&template_frame, roi).unwrap();
        let search_frame = frame_with_square(2, 117, 91, 25, 25);
        let detection = detector.detect(&search_frame).unwrap().unwrap();
        let bbox = detection.bbox.unwrap();
        assert!((bbox.x - 117.0).abs() <= 1.0);
        assert!((bbox.y - 91.0).abs() <= 1.0);
        assert!(detection.confidence > 0.9);
    }

    #[test]
    fn ringgrid_detector_reports_synthetic_marker_centers_in_image_pixels() {
        let config = RingGridTargetConfig {
            rows: 3,
            long_row_cols: 3,
            pitch_mm: 8.0,
            outer_radius_mm: 2.4,
            inner_radius_mm: 1.4,
            ring_width_mm: 0.5,
        };
        let target = ringgrid::TargetLayout::coded_hex(
            config.pitch_mm,
            usize::from(config.rows),
            usize::from(config.long_row_cols),
            config.outer_radius_mm,
            config.inner_radius_mm,
            config.ring_width_mm,
        )
        .unwrap();
        let image = target
            .render_target_png(&ringgrid::PngTargetOptions {
                dpi: 180.0,
                margin_mm: 4.0,
                include_scale_bar: false,
            })
            .unwrap();
        let frame = Frame {
            meta: FrameMeta {
                frame_id: 7,
                timestamp: now(),
                width: image.width(),
                height: image.height(),
                stride: image.width(),
                pixel_format: PixelFormat::Gray8,
            },
            bytes: Bytes::from(image.into_raw()),
        };
        let mut detector = RingGridDetector::new(&config).unwrap();
        let detection = detector.detect(&frame).unwrap().unwrap();

        assert_eq!(detection.method, AlgorithmId::RingGridTarget);
        assert!(!detection.points.is_empty());
        assert!(detection.points.iter().all(|point| {
            point.x >= 0.0
                && point.x < frame.meta.width as f32
                && point.y >= 0.0
                && point.y < frame.meta.height as f32
        }));
        assert!(detection.bbox.is_some());
        assert!(detection.confidence > 0.0);
    }

    #[test]
    fn ringgrid_frame_conversion_removes_stride_padding_and_converts_rgb() {
        let gray = Frame {
            meta: FrameMeta {
                frame_id: 1,
                timestamp: now(),
                width: 3,
                height: 2,
                stride: 4,
                pixel_format: PixelFormat::Gray8,
            },
            bytes: Bytes::from_static(&[1, 2, 3, 99, 4, 5, 6, 99]),
        };
        assert_eq!(
            frame_to_gray_image(&gray).unwrap().as_raw(),
            &[1, 2, 3, 4, 5, 6]
        );

        let rgb = Frame {
            meta: FrameMeta {
                frame_id: 2,
                timestamp: now(),
                width: 2,
                height: 1,
                stride: 7,
                pixel_format: PixelFormat::Rgb8,
            },
            bytes: Bytes::from_static(&[255, 0, 0, 0, 255, 0, 99]),
        };
        assert_eq!(frame_to_gray_image(&rgb).unwrap().as_raw(), &[77, 149]);
    }

    #[test]
    fn ringgrid_rejects_invalid_target_without_constructing_detector() {
        let config = RingGridTargetConfig {
            inner_radius_mm: 4.8,
            ..RingGridTargetConfig::default()
        };
        assert!(RingGridDetector::new(&config).is_err());
    }

    fn frame_with_square(frame_id: u64, x0: u32, y0: u32, w: u32, h: u32) -> Frame {
        let width = 180;
        let height = 140;
        let mut bytes = vec![20_u8; width * height];
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                let value = if (x + y) % 2 == 0 { 240 } else { 180 };
                bytes[(y * width as u32 + x) as usize] = value;
            }
        }
        Frame {
            meta: FrameMeta {
                frame_id,
                timestamp: now(),
                width: width as u32,
                height: height as u32,
                stride: width as u32,
                pixel_format: PixelFormat::Gray8,
            },
            bytes: Bytes::from(bytes),
        }
    }
}
