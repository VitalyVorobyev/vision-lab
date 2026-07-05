use super::{
    CameraComponent, ensure_active_selection, fps_since, publish_event, select_device,
    select_format, store_state,
};
use block2::RcBlock;
use bytes::Bytes;
use comm_core::{
    ApiError, CommandReceipt, ComponentIdentity, OperationId, Versioned, now, versioned,
};
use comm_local::{
    CommandInbox, EventBus, LatestValueBus, MonotonicCounter, StateCell, command_channel,
};
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, Bool, NSObject, ProtocolObject};
use objc2::{DefinedClass, define_class, extern_methods, msg_send};
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceDiscoverySession,
    AVCaptureDeviceFormat, AVCaptureDeviceInput, AVCaptureDevicePosition, AVCaptureDeviceType,
    AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeContinuityCamera,
    AVCaptureDeviceTypeExternal, AVCaptureOutput, AVCaptureSession,
    AVCaptureSessionPresetInputPriority, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferGetWidthOfPlane,
    CVPixelBufferIsPlanar, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_24BGR, kCVPixelFormatType_24RGB,
    kCVPixelFormatType_32ABGR, kCVPixelFormatType_32ARGB, kCVPixelFormatType_32BGRA,
    kCVPixelFormatType_422YpCbCr8, kCVPixelFormatType_422YpCbCr8_yuvs,
    kCVPixelFormatType_422YpCbCr8FullRange, kCVPixelFormatType_OneComponent8, kCVReturnSuccess,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSString, ns_string};
use std::ffi::CStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use vision_contracts::{
    CameraCommand, CameraCommandKind, CameraDeviceInfo, CameraEvent, CameraFormatInfo,
    CameraLifecycle, CameraPermissionStatus, CameraPosition, CameraState, CameraTransport, Frame,
    FrameMeta, PixelFormat,
};

type NativeUpdateTx = mpsc::UnboundedSender<NativeCameraUpdate>;

#[derive(Debug, Clone)]
enum NativeCameraUpdate {
    Frame(NativeFrameMeta),
    Dropped { dropped_frames: u64 },
}

#[derive(Debug, Clone)]
struct NativeFrameMeta {
    frame_id: u64,
    width: u32,
    height: u32,
}

pub fn spawn(component_name: &str) -> Arc<CameraComponent> {
    let identity = ComponentIdentity::new("camera", component_name, env!("CARGO_PKG_VERSION"));
    let initial = initial_state(&identity);
    let state = Arc::new(StateCell::new(initial));
    let events = Arc::new(EventBus::new(256));
    let frames = Arc::new(LatestValueBus::new());
    let (client, inbox) = command_channel(32);
    let component = Arc::new(CameraComponent {
        client,
        state: state.clone(),
        events: events.clone(),
        frames: frames.clone(),
    });

    std::thread::spawn(move || run_camera_thread(identity, inbox, state, events, frames));
    component
}

fn run_camera_thread(
    identity: ComponentIdentity,
    inbox: CommandInbox<CameraCommand, CommandReceipt>,
    state: Arc<StateCell<Versioned<CameraState>>>,
    events: Arc<EventBus<CameraEvent>>,
    frames: Arc<LatestValueBus<Frame>>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to start native camera runtime");
    runtime.block_on(run_camera(identity, inbox, state, events, frames));
}

fn initial_state(identity: &ComponentIdentity) -> Versioned<CameraState> {
    let mut state = CameraState {
        permission_status: permission_status(),
        available_devices: discover_devices(),
        ..CameraState::default()
    };
    let _ = ensure_active_selection(&mut state);
    versioned(identity.clone(), 0, state)
}

async fn run_camera(
    identity: ComponentIdentity,
    mut inbox: CommandInbox<CameraCommand, CommandReceipt>,
    state_cell: Arc<StateCell<Versioned<CameraState>>>,
    events: Arc<EventBus<CameraEvent>>,
    frames: Arc<LatestValueBus<Frame>>,
) {
    let sequence = MonotonicCounter::default();
    let mut revision = MonotonicCounter::default();
    let mut camera = NativeCamera::default();
    let mut state = initial_state(&identity).value;
    let (update_tx, mut update_rx) = mpsc::unbounded_channel();
    let mut last_frame_at = tokio::time::Instant::now();

    loop {
        tokio::select! {
            Some(update) = update_rx.recv() => {
                match update {
                    NativeCameraUpdate::Frame(meta) => {
                        update_frame_state(&mut state, meta, &mut last_frame_at);
                        publish_frame_event(&identity, &events, &sequence, state.frame_id);
                    }
                    NativeCameraUpdate::Dropped { dropped_frames } => {
                        state.dropped_frames = dropped_frames;
                        publish_dropped_event(&identity, &events, &sequence, dropped_frames);
                    }
                }
                store_state(&identity, &state_cell, &mut revision, state.clone()).await;
            }
            Some(request) = inbox.recv() => {
                let command = request.command;
                let result = handle_command(
                    &mut camera,
                    &mut state,
                    &command,
                    update_tx.clone(),
                    frames.clone(),
                );
                if result.is_ok() {
                    publish_command_event(&identity, &events, &sequence, &command, &state);
                }
                let result = store_command_state(result, &identity, &state_cell, &mut revision, &state, &command).await;
                let _ = request.respond_to.send(result);
            }
            else => break,
        }
    }
}

fn publish_command_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    command: &CameraCommand,
    state: &CameraState,
) {
    let payload = match &command.kind {
        CameraCommandKind::RefreshDevices => Some(CameraEvent::DevicesChanged {
            count: state.available_devices.len(),
        }),
        CameraCommandKind::Connect
        | CameraCommandKind::StartStream
        | CameraCommandKind::StopStream => Some(CameraEvent::LifecycleChanged {
            lifecycle: state.lifecycle,
        }),
        CameraCommandKind::SelectDevice { .. } | CameraCommandKind::SelectFormat { .. } => {
            Some(CameraEvent::ActiveConfigChanged {
                device_id: state.active_device_id.clone(),
                format_id: state.active_format_id.clone(),
            })
        }
        CameraCommandKind::SetRequestedFps { fps } => {
            Some(CameraEvent::RequestedFpsChanged { fps: *fps })
        }
    };
    if let Some(payload) = payload {
        publish_event(identity, events, sequence, command.correlation_id, payload);
    }
}

fn update_frame_state(
    state: &mut CameraState,
    meta: NativeFrameMeta,
    last_frame_at: &mut tokio::time::Instant,
) {
    state.frame_id = meta.frame_id;
    state.frame_width = meta.width;
    state.frame_height = meta.height;
    state.actual_fps = fps_since(last_frame_at);
}

fn publish_frame_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    frame_id: u64,
) {
    publish_event(
        identity,
        events,
        sequence,
        None,
        CameraEvent::FrameProduced { frame_id },
    );
}

fn publish_dropped_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    dropped_frames: u64,
) {
    publish_event(
        identity,
        events,
        sequence,
        None,
        CameraEvent::DroppedFramesChanged { dropped_frames },
    );
}

async fn store_command_state(
    result: Result<(), ApiError>,
    identity: &ComponentIdentity,
    state_cell: &StateCell<Versioned<CameraState>>,
    revision: &mut MonotonicCounter,
    state: &CameraState,
    command: &CameraCommand,
) -> Result<CommandReceipt, ApiError> {
    result?;
    let accepted_revision = store_state(identity, state_cell, revision, state.clone()).await;
    Ok(CommandReceipt {
        command_id: command.command_id,
        operation_id: Some(OperationId::new()),
        accepted_revision,
    })
}

fn handle_command(
    camera: &mut NativeCamera,
    state: &mut CameraState,
    command: &CameraCommand,
    update_tx: NativeUpdateTx,
    frames: Arc<LatestValueBus<Frame>>,
) -> Result<(), ApiError> {
    match &command.kind {
        CameraCommandKind::RefreshDevices => camera.refresh_devices(state),
        CameraCommandKind::Connect => camera.connect(state),
        CameraCommandKind::SelectDevice { device_id } => {
            select_device(state, device_id)?;
            camera.restart_stream_if_running(state, update_tx, frames)
        }
        CameraCommandKind::SelectFormat { format_id } => {
            select_format(state, format_id)?;
            camera.restart_stream_if_running(state, update_tx, frames)
        }
        CameraCommandKind::StartStream => camera.start_stream(state, update_tx, frames),
        CameraCommandKind::StopStream => camera.stop_stream(state),
        CameraCommandKind::SetRequestedFps { fps } => {
            set_requested_fps(state, *fps)?;
            camera.restart_stream_if_running(state, update_tx, frames)
        }
    }
}

#[derive(Default)]
struct NativeCamera {
    capture: Option<CaptureRuntime>,
}

impl NativeCamera {
    fn refresh_devices(&mut self, state: &mut CameraState) -> Result<(), ApiError> {
        state.available_devices = discover_devices();
        state.permission_status = permission_status();
        ensure_active_selection(state)
    }

    fn connect(&mut self, state: &mut CameraState) -> Result<(), ApiError> {
        state.lifecycle = CameraLifecycle::Connecting;
        state.permission_status = request_permission();
        if state.permission_status != CameraPermissionStatus::Authorized {
            return permission_error(state);
        }
        self.refresh_devices(state)?;
        state.lifecycle = CameraLifecycle::Ready;
        state.error = None;
        Ok(())
    }

    fn start_stream(
        &mut self,
        state: &mut CameraState,
        update_tx: NativeUpdateTx,
        frames: Arc<LatestValueBus<Frame>>,
    ) -> Result<(), ApiError> {
        ensure_active_selection(state)?;
        if state.permission_status != CameraPermissionStatus::Authorized {
            return permission_error(state);
        }
        if state.lifecycle == CameraLifecycle::Disconnected {
            return Err(ApiError::Rejected(
                "camera must be connected before streaming".into(),
            ));
        }
        if let Some(mut capture) = self.capture.take() {
            capture.stop();
        }
        let capture = CaptureRuntime::start(state, update_tx, frames)?;
        state.lifecycle = CameraLifecycle::Streaming;
        state.error = None;
        self.capture = Some(capture);
        Ok(())
    }

    fn restart_stream_if_running(
        &mut self,
        state: &mut CameraState,
        update_tx: NativeUpdateTx,
        frames: Arc<LatestValueBus<Frame>>,
    ) -> Result<(), ApiError> {
        if state.lifecycle != CameraLifecycle::Streaming {
            return Ok(());
        }
        self.start_stream(state, update_tx, frames)
    }

    fn stop_stream(&mut self, state: &mut CameraState) -> Result<(), ApiError> {
        if let Some(mut capture) = self.capture.take() {
            capture.stop();
        }
        state.lifecycle = CameraLifecycle::Ready;
        state.actual_fps = 0.0;
        Ok(())
    }
}

struct CaptureRuntime {
    session: Retained<AVCaptureSession>,
    output: Retained<AVCaptureVideoDataOutput>,
    _delegate: Retained<FrameDelegate>,
    _queue: DispatchRetained<DispatchQueue>,
}

impl CaptureRuntime {
    fn start(
        state: &CameraState,
        update_tx: NativeUpdateTx,
        frames: Arc<LatestValueBus<Frame>>,
    ) -> Result<Self, ApiError> {
        let device = selected_device(state)?;
        let shared = Arc::new(DelegateShared::new(frames, update_tx, state.dropped_frames));
        let runtime = configure_session(&device, state, shared)?;
        unsafe { runtime.session.startRunning() };
        Ok(runtime)
    }

    fn stop(&mut self) {
        unsafe {
            self.session.stopRunning();
            self.output.setSampleBufferDelegate_queue(None, None);
        }
    }
}

fn configure_session(
    device: &AVCaptureDevice,
    state: &CameraState,
    shared: Arc<DelegateShared>,
) -> Result<CaptureRuntime, ApiError> {
    let requested_format = requested_format(device, state)?;
    let input = device_input(device)?;
    let session = unsafe { AVCaptureSession::new() };
    let output = unsafe { AVCaptureVideoDataOutput::new() };
    let delegate = FrameDelegate::new(shared);
    let queue = DispatchQueue::new(
        "com.vitavision.vision-lab.camera",
        DispatchQueueAttr::SERIAL,
    );
    unsafe {
        output.setAlwaysDiscardsLateVideoFrames(true);
        configure_video_output_format(&output);
        output.setSampleBufferDelegate_queue(
            Some(ProtocolObject::from_ref(&*delegate)),
            Some(&queue),
        );
        configure_inputs_outputs(
            &session,
            device,
            &input,
            &output,
            requested_format.as_deref(),
            state,
        )?;
    }
    Ok(CaptureRuntime {
        session,
        output,
        _delegate: delegate,
        _queue: queue,
    })
}

unsafe fn configure_inputs_outputs(
    session: &AVCaptureSession,
    device: &AVCaptureDevice,
    input: &AVCaptureDeviceInput,
    output: &AVCaptureVideoDataOutput,
    requested_format: Option<&AVCaptureDeviceFormat>,
    state: &CameraState,
) -> Result<(), ApiError> {
    unsafe {
        session.beginConfiguration();
        if session.canSetSessionPreset(AVCaptureSessionPresetInputPriority) {
            session.setSessionPreset(AVCaptureSessionPresetInputPriority);
        }
        if !session.canAddInput(input) {
            return Err(ApiError::Rejected(
                "selected camera cannot be added to session".into(),
            ));
        }
        session.addInput(input);
        if !session.canAddOutput(output) {
            return Err(ApiError::Rejected(
                "video output cannot be added to session".into(),
            ));
        }
        session.addOutput(output);
        apply_requested_format(device, requested_format, state)?;
        session.commitConfiguration();
    }
    Ok(())
}

fn device_input(device: &AVCaptureDevice) -> Result<Retained<AVCaptureDeviceInput>, ApiError> {
    unsafe { AVCaptureDeviceInput::deviceInputWithDevice_error(device) }.map_err(ns_error)
}

fn requested_format(
    device: &AVCaptureDevice,
    state: &CameraState,
) -> Result<Option<Retained<AVCaptureDeviceFormat>>, ApiError> {
    let Some(format_id) = state.active_format_id.as_deref() else {
        return Ok(None);
    };
    let Some((format, _)) = find_format(device, format_id) else {
        return Err(ApiError::InvalidRequest(format!(
            "camera format not available on active device: {format_id}"
        )));
    };
    Ok(Some(format))
}

unsafe fn apply_requested_format(
    device: &AVCaptureDevice,
    format: Option<&AVCaptureDeviceFormat>,
    state: &CameraState,
) -> Result<(), ApiError> {
    let Some(format) = format else {
        return Ok(());
    };
    unsafe {
        device.lockForConfiguration().map_err(ns_error)?;
        device.setActiveFormat(format);
        let duration = CMTime::new(1, fps_for_format(format, state));
        device.setActiveVideoMinFrameDuration(duration);
        device.setActiveVideoMaxFrameDuration(duration);
        device.unlockForConfiguration();
    }
    Ok(())
}

fn fps_for_format(format: &AVCaptureDeviceFormat, state: &CameraState) -> i32 {
    let (min_fps, max_fps) = unsafe { fps_range(format) };
    let mut fps = state.requested_fps;
    if max_fps > 0.0 {
        fps = fps.clamp(min_fps.max(1.0), max_fps);
    }
    fps.round().clamp(1.0, 120.0) as i32
}

unsafe fn configure_video_output_format(output: &AVCaptureVideoDataOutput) {
    let available = unsafe { output.availableVideoCVPixelFormatTypes() };
    let preferred = preferred_output_format(&available);
    match preferred {
        Some(format) => {
            let key = ns_string!("PixelFormatType");
            let value = NSNumber::new_u32(format);
            let value_ref: &AnyObject = value.as_ref();
            let settings = NSDictionary::from_slices(&[key], &[value_ref]);
            unsafe { output.setVideoSettings(Some(&settings)) };
        }
        None => {
            let settings: Retained<NSDictionary<NSString, AnyObject>> =
                NSDictionary::from_slices::<NSString>(&[], &[]);
            unsafe { output.setVideoSettings(Some(&settings)) };
        }
    }
}

fn preferred_output_format(available: &NSArray<NSNumber>) -> Option<u32> {
    let formats: Vec<u32> = available
        .to_vec()
        .iter()
        .map(|format| format.as_u32())
        .collect();
    if formats.contains(&kCVPixelFormatType_OneComponent8) {
        Some(kCVPixelFormatType_OneComponent8)
    } else if formats.contains(&kCVPixelFormatType_32BGRA) {
        Some(kCVPixelFormatType_32BGRA)
    } else {
        None
    }
}

fn set_requested_fps(state: &mut CameraState, fps: f32) -> Result<(), ApiError> {
    if !(1.0..=120.0).contains(&fps) {
        return Err(ApiError::InvalidRequest(
            "requested fps must be in the range 1..=120".into(),
        ));
    }
    state.requested_fps = fps;
    Ok(())
}

fn permission_error(state: &mut CameraState) -> Result<(), ApiError> {
    state.lifecycle = CameraLifecycle::Error;
    state.error = Some("camera permission is not authorized".into());
    Err(ApiError::Rejected(
        "camera permission is not authorized".into(),
    ))
}

fn discover_devices() -> Vec<CameraDeviceInfo> {
    unsafe {
        let types = discovery_types();
        let session =
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &types,
                AVMediaTypeVideo,
                AVCaptureDevicePosition::Unspecified,
            );
        session
            .devices()
            .to_vec()
            .into_iter()
            .enumerate()
            .map(|(index, device)| device_info(&device, index == 0))
            .collect()
    }
}

unsafe fn discovery_types() -> Retained<NSArray<AVCaptureDeviceType>> {
    unsafe {
        NSArray::from_slice(&[
            AVCaptureDeviceTypeBuiltInWideAngleCamera,
            AVCaptureDeviceTypeContinuityCamera,
            AVCaptureDeviceTypeExternal,
        ])
    }
}

unsafe fn device_info(device: &AVCaptureDevice, is_default: bool) -> CameraDeviceInfo {
    let unique_id = unsafe { device.uniqueID() };
    let display_name = unsafe { device.localizedName() };
    let device_type = unsafe { device.deviceType() };
    CameraDeviceInfo {
        id: ns_string(&unique_id),
        display_name: ns_string(&display_name),
        model_id: optional_string(unsafe { device.modelID() }),
        manufacturer: optional_string(unsafe { device.manufacturer() }),
        position: position(unsafe { device.position() }, device_type.as_ref()),
        transport: transport(device_type.as_ref()),
        is_default,
        formats: unsafe { formats(device) },
    }
}

fn optional_string(value: Retained<NSString>) -> Option<String> {
    let text = ns_string(&value);
    if text.is_empty() { None } else { Some(text) }
}

unsafe fn formats(device: &AVCaptureDevice) -> Vec<CameraFormatInfo> {
    unsafe { device.formats() }
        .to_vec()
        .into_iter()
        .enumerate()
        .map(|(index, format)| unsafe { format_info(device, &format, index) })
        .collect()
}

unsafe fn format_info(
    device: &AVCaptureDevice,
    format: &AVCaptureDeviceFormat,
    index: usize,
) -> CameraFormatInfo {
    let description = unsafe { format.formatDescription() };
    let dimensions =
        unsafe { objc2_core_media::CMVideoFormatDescriptionGetDimensions(&description) };
    let (min_fps, max_fps) = unsafe { fps_range(format) };
    CameraFormatInfo {
        id: format_id(device, index),
        width: dimensions.width.max(0) as u32,
        height: dimensions.height.max(0) as u32,
        pixel_format: pixel_format(unsafe { description.media_sub_type() }),
        min_fps,
        max_fps,
    }
}

unsafe fn fps_range(format: &AVCaptureDeviceFormat) -> (f32, f32) {
    let mut min_fps = f64::MAX;
    let mut max_fps = 0.0_f64;
    for range in unsafe { format.videoSupportedFrameRateRanges() }.to_vec() {
        min_fps = min_fps.min(unsafe { range.minFrameRate() });
        max_fps = max_fps.max(unsafe { range.maxFrameRate() });
    }
    if max_fps <= 0.0 {
        (0.0, 0.0)
    } else {
        (min_fps as f32, max_fps as f32)
    }
}

fn pixel_format(subtype: u32) -> PixelFormat {
    if subtype == kCVPixelFormatType_OneComponent8 {
        PixelFormat::Gray8
    } else {
        PixelFormat::Rgb8
    }
}

fn format_id(device: &AVCaptureDevice, index: usize) -> String {
    let id = unsafe { ns_string(&device.uniqueID()) };
    format!("{id}:format:{index}")
}

fn position(
    position: AVCaptureDevicePosition,
    device_type: &AVCaptureDeviceType,
) -> CameraPosition {
    if position == AVCaptureDevicePosition::Front {
        CameraPosition::Front
    } else if position == AVCaptureDevicePosition::Back {
        CameraPosition::Back
    } else if is_same_device_type(device_type, unsafe { AVCaptureDeviceTypeExternal }) {
        CameraPosition::External
    } else {
        CameraPosition::Unknown
    }
}

fn transport(device_type: &AVCaptureDeviceType) -> CameraTransport {
    if is_same_device_type(device_type, unsafe { AVCaptureDeviceTypeContinuityCamera }) {
        CameraTransport::Continuity
    } else if is_same_device_type(device_type, unsafe { AVCaptureDeviceTypeExternal }) {
        CameraTransport::Usb
    } else {
        CameraTransport::BuiltIn
    }
}

fn is_same_device_type(left: &AVCaptureDeviceType, right: &AVCaptureDeviceType) -> bool {
    left.isEqualToString(right)
}

fn selected_device(state: &CameraState) -> Result<Retained<AVCaptureDevice>, ApiError> {
    let device_id = state
        .active_device_id
        .as_deref()
        .ok_or_else(|| ApiError::InvalidRequest("no active camera device".into()))?;
    discover_device(device_id)
        .ok_or_else(|| ApiError::InvalidRequest(format!("camera device not found: {device_id}")))
}

fn discover_device(device_id: &str) -> Option<Retained<AVCaptureDevice>> {
    unsafe {
        let types = discovery_types();
        let session =
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &types,
                AVMediaTypeVideo,
                AVCaptureDevicePosition::Unspecified,
            );
        session
            .devices()
            .to_vec()
            .into_iter()
            .find(|device| ns_string(&device.uniqueID()) == device_id)
    }
}

fn find_format(
    device: &AVCaptureDevice,
    format_id_value: &str,
) -> Option<(Retained<AVCaptureDeviceFormat>, usize)> {
    unsafe {
        device
            .formats()
            .to_vec()
            .into_iter()
            .enumerate()
            .find(|(index, _)| format_id(device, *index) == format_id_value)
            .map(|(index, format)| (format, index))
    }
}

fn request_permission() -> CameraPermissionStatus {
    let status = permission_status();
    if status != CameraPermissionStatus::NotDetermined {
        return status;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let block = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });
    unsafe {
        if let Some(media_type) = AVMediaTypeVideo {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &block);
        }
    }
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(true) => CameraPermissionStatus::Authorized,
        Ok(false) => CameraPermissionStatus::Denied,
        Err(_) => permission_status(),
    }
}

fn permission_status() -> CameraPermissionStatus {
    unsafe {
        let Some(media_type) = AVMediaTypeVideo else {
            return CameraPermissionStatus::Unknown;
        };
        match AVCaptureDevice::authorizationStatusForMediaType(media_type) {
            AVAuthorizationStatus::NotDetermined => CameraPermissionStatus::NotDetermined,
            AVAuthorizationStatus::Restricted => CameraPermissionStatus::Restricted,
            AVAuthorizationStatus::Denied => CameraPermissionStatus::Denied,
            AVAuthorizationStatus::Authorized => CameraPermissionStatus::Authorized,
            _ => CameraPermissionStatus::Unknown,
        }
    }
}

fn ns_error(error: Retained<NSError>) -> ApiError {
    ApiError::Rejected(ns_string(&error.localizedDescription()))
}

fn ns_string(value: &NSString) -> String {
    let pointer = value.UTF8String();
    if pointer.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

struct DelegateShared {
    frames: Arc<LatestValueBus<Frame>>,
    update_tx: NativeUpdateTx,
    frame_id: std::sync::atomic::AtomicU64,
    dropped_frames: std::sync::atomic::AtomicU64,
}

impl DelegateShared {
    fn new(
        frames: Arc<LatestValueBus<Frame>>,
        update_tx: NativeUpdateTx,
        initial_dropped_frames: u64,
    ) -> Self {
        Self {
            frames,
            update_tx,
            frame_id: std::sync::atomic::AtomicU64::new(0),
            dropped_frames: std::sync::atomic::AtomicU64::new(initial_dropped_frames),
        }
    }

    fn publish(&self, frame: Frame) {
        let meta = NativeFrameMeta {
            frame_id: frame.meta.frame_id,
            width: frame.meta.width,
            height: frame.meta.height,
        };
        self.frames.publish(frame);
        let _ = self.update_tx.send(NativeCameraUpdate::Frame(meta));
    }

    fn publish_drop(&self) {
        let dropped_frames = self
            .dropped_frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1);
        let _ = self
            .update_tx
            .send(NativeCameraUpdate::Dropped { dropped_frames });
    }
}

struct FrameDelegateIvars {
    shared: Arc<DelegateShared>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = FrameDelegateIvars]
    struct FrameDelegate;

    impl FrameDelegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn did_output(
            &self,
            _output: &AVCaptureOutput,
            sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            if let Some(frame) = unsafe { sample_frame(sample_buffer, &self.ivars().shared) } {
                self.ivars().shared.publish(frame);
            }
        }

        #[unsafe(method(captureOutput:didDropSampleBuffer:fromConnection:))]
        unsafe fn did_drop(
            &self,
            _output: &AVCaptureOutput,
            _sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            self.ivars().shared.publish_drop();
        }
    }
);

unsafe impl objc2_foundation::NSObjectProtocol for FrameDelegate {}
unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for FrameDelegate {}

impl FrameDelegate {
    extern_methods!(
        #[unsafe(method(alloc))]
        fn alloc() -> Allocated<Self>;
    );

    fn new(shared: Arc<DelegateShared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(FrameDelegateIvars { shared });
        unsafe { msg_send![super(this), init] }
    }
}

unsafe fn sample_frame(sample_buffer: &CMSampleBuffer, shared: &DelegateShared) -> Option<Frame> {
    let pixel_buffer = unsafe { sample_buffer.image_buffer() }?;
    let locked =
        unsafe { CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly) };
    if locked != kCVReturnSuccess {
        return None;
    }
    let result = unsafe { copy_pixel_buffer(&pixel_buffer, shared) };
    let _ =
        unsafe { CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly) };
    result
}

unsafe fn copy_pixel_buffer(
    pixel_buffer: &objc2_core_video::CVPixelBuffer,
    shared: &DelegateShared,
) -> Option<Frame> {
    if CVPixelBufferIsPlanar(pixel_buffer) {
        unsafe { copy_luma_plane(pixel_buffer, shared) }
    } else {
        unsafe { copy_packed_pixels(pixel_buffer, shared) }
    }
}

unsafe fn copy_luma_plane(
    pixel_buffer: &objc2_core_video::CVPixelBuffer,
    shared: &DelegateShared,
) -> Option<Frame> {
    let width = CVPixelBufferGetWidthOfPlane(pixel_buffer, 0) as u32;
    let height = CVPixelBufferGetHeightOfPlane(pixel_buffer, 0) as u32;
    let stride = CVPixelBufferGetBytesPerRowOfPlane(pixel_buffer, 0);
    let base = CVPixelBufferGetBaseAddressOfPlane(pixel_buffer, 0) as *const u8;
    unsafe { copy_gray(base, width, height, stride, shared) }
}

unsafe fn copy_packed_pixels(
    pixel_buffer: &objc2_core_video::CVPixelBuffer,
    shared: &DelegateShared,
) -> Option<Frame> {
    let width = CVPixelBufferGetWidth(pixel_buffer) as u32;
    let height = CVPixelBufferGetHeight(pixel_buffer) as u32;
    let stride = CVPixelBufferGetBytesPerRow(pixel_buffer);
    let base = CVPixelBufferGetBaseAddress(pixel_buffer) as *const u8;
    let format = CVPixelBufferGetPixelFormatType(pixel_buffer);
    if format == kCVPixelFormatType_OneComponent8 {
        unsafe { copy_gray(base, width, height, stride, shared) }
    } else {
        unsafe { copy_rgb_like(base, width, height, stride, format, shared) }
    }
}

unsafe fn copy_gray(
    base: *const u8,
    width: u32,
    height: u32,
    stride: usize,
    shared: &DelegateShared,
) -> Option<Frame> {
    if base.is_null() {
        return None;
    }
    let total_len = stride.checked_mul(height as usize)?;
    let source = unsafe { std::slice::from_raw_parts(base, total_len) };
    let bytes = copy_padded_gray_rows(source, width as usize, height as usize, stride)?;
    Some(frame(bytes, width, height, shared))
}

fn copy_padded_gray_rows(
    source: &[u8],
    width: usize,
    height: usize,
    stride: usize,
) -> Option<Vec<u8>> {
    if stride < width || source.len() < stride.checked_mul(height)? {
        return None;
    }
    let mut bytes = vec![0_u8; width.checked_mul(height)?];
    for row in 0..height {
        let source_start = row * stride;
        let target_start = row * width;
        bytes[target_start..target_start + width]
            .copy_from_slice(&source[source_start..source_start + width]);
    }
    Some(bytes)
}

unsafe fn copy_rgb_like(
    base: *const u8,
    width: u32,
    height: u32,
    stride: usize,
    format: u32,
    shared: &DelegateShared,
) -> Option<Frame> {
    if base.is_null() {
        return None;
    }
    let mut gray = vec![0_u8; (width * height) as usize];
    for row in 0..height as usize {
        let source = unsafe { std::slice::from_raw_parts(base.add(row * stride), stride) };
        convert_row_to_gray(source, &mut gray, row, width as usize, format);
    }
    Some(frame(gray, width, height, shared))
}

fn convert_row_to_gray(source: &[u8], gray: &mut [u8], row: usize, width: usize, format: u32) {
    if is_packed_yuv_422(format) {
        convert_packed_yuv_422_row_to_gray(source, gray, row, width, format);
        return;
    }
    for col in 0..width {
        let offset = col * pixel_stride(format);
        let (r, g, b) = rgb_triplet(source, offset, format);
        gray[row * width + col] = luminance(r, g, b);
    }
}

fn pixel_stride(format: u32) -> usize {
    if format == kCVPixelFormatType_24RGB || format == kCVPixelFormatType_24BGR {
        3
    } else {
        4
    }
}

fn rgb_triplet(source: &[u8], offset: usize, format: u32) -> (u8, u8, u8) {
    let first = source.get(offset).copied().unwrap_or(0);
    let second = source.get(offset + 1).copied().unwrap_or(0);
    let third = source.get(offset + 2).copied().unwrap_or(0);
    let fourth = source.get(offset + 3).copied().unwrap_or(0);
    if format == kCVPixelFormatType_32BGRA || format == kCVPixelFormatType_24BGR {
        (third, second, first)
    } else if format == kCVPixelFormatType_32ARGB {
        (second, third, fourth)
    } else if format == kCVPixelFormatType_32ABGR {
        (fourth, third, second)
    } else {
        (first, second, third)
    }
}

fn luminance(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 77 + u32::from(g) * 150 + u32::from(b) * 29) >> 8) as u8
}

fn is_packed_yuv_422(format: u32) -> bool {
    format == kCVPixelFormatType_422YpCbCr8
        || format == kCVPixelFormatType_422YpCbCr8_yuvs
        || format == kCVPixelFormatType_422YpCbCr8FullRange
}

fn convert_packed_yuv_422_row_to_gray(
    source: &[u8],
    gray: &mut [u8],
    row: usize,
    width: usize,
    format: u32,
) {
    for col in 0..width {
        let pair_offset = (col / 2) * 4;
        let y_offset = if format == kCVPixelFormatType_422YpCbCr8 {
            if col % 2 == 0 { 1 } else { 3 }
        } else if col % 2 == 0 {
            0
        } else {
            2
        };
        gray[row * width + col] = source.get(pair_offset + y_offset).copied().unwrap_or(0);
    }
}

fn frame(bytes: Vec<u8>, width: u32, height: u32, shared: &DelegateShared) -> Frame {
    let frame_id = shared
        .frame_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .saturating_add(1);
    Frame {
        meta: FrameMeta {
            frame_id,
            timestamp: now(),
            width,
            height,
            stride: width,
            pixel_format: PixelFormat::Gray8,
        },
        bytes: Bytes::from(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_padded_gray_rows_removes_stride_padding() {
        let source = [1, 2, 3, 99, 4, 5, 6, 88];
        let gray = copy_padded_gray_rows(&source, 3, 2, 4).unwrap();
        assert_eq!(gray, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn bgra_row_converts_to_luma() {
        let mut gray = vec![0; 2];
        let source = [
            0, 0, 255, 255, // red
            255, 0, 0, 255, // blue
        ];
        convert_row_to_gray(&source, &mut gray, 0, 2, kCVPixelFormatType_32BGRA);
        assert_eq!(gray, vec![76, 28]);
    }

    #[test]
    fn rgb_and_bgr_rows_use_channel_order() {
        let mut rgb_gray = vec![0; 1];
        let mut bgr_gray = vec![0; 1];
        convert_row_to_gray(&[255, 0, 0], &mut rgb_gray, 0, 1, kCVPixelFormatType_24RGB);
        convert_row_to_gray(&[0, 0, 255], &mut bgr_gray, 0, 1, kCVPixelFormatType_24BGR);
        assert_eq!(rgb_gray, vec![76]);
        assert_eq!(bgr_gray, vec![76]);
    }

    #[test]
    fn planar_luma_copy_matches_packed_gray_contract() {
        let source = [10, 11, 12, 0, 20, 21, 22, 0];
        let gray = copy_padded_gray_rows(&source, 3, 2, 4).unwrap();
        assert_eq!(gray, vec![10, 11, 12, 20, 21, 22]);
    }

    #[test]
    fn packed_2vuy_extracts_y_samples() {
        let mut gray = vec![0; 4];
        let source = [
            128, 10, 64, 20, // Cb Y0 Cr Y1
            128, 30, 64, 40,
        ];
        convert_row_to_gray(&source, &mut gray, 0, 4, kCVPixelFormatType_422YpCbCr8);
        assert_eq!(gray, vec![10, 20, 30, 40]);
    }

    #[test]
    fn packed_yuvs_extracts_y_samples() {
        let mut gray = vec![0; 4];
        let source = [
            10, 128, 20, 64, // Y0 Cb Y1 Cr
            30, 128, 40, 64,
        ];
        convert_row_to_gray(&source, &mut gray, 0, 4, kCVPixelFormatType_422YpCbCr8_yuvs);
        assert_eq!(gray, vec![10, 20, 30, 40]);
    }
}
