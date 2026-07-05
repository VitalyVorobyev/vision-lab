//! Camera component runtime.
//!
//! The v1 implementation provides a deterministic simulator behind the same
//! API planned for the real macOS camera bridge. The AVFoundation bridge stays
//! isolated behind the `real-camera` feature and is intentionally not exposed
//! through public contracts.

use async_trait::async_trait;
use bytes::Bytes;
use comm_core::{
    ApiError, CommandReceipt, ComponentIdentity, OperationId, Versioned, event, now, versioned,
};
use comm_local::{
    CommandClient, CommandInbox, EventBus, LatestValueBus, MonotonicCounter, StateCell,
    command_channel,
};
use std::{sync::Arc, time::Duration};
use tokio::time::{Instant, interval};
use vision_contracts::{
    CameraApi, CameraCommand, CameraCommandKind, CameraDeviceInfo, CameraEvent, CameraEventStream,
    CameraFormatInfo, CameraLifecycle, CameraPermissionStatus, CameraPosition, CameraState,
    CameraTransport, Frame, FrameMeta, FrameStream, PixelFormat,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const SIM_DEVICE_ID: &str = "simulated-camera";
const SIM_FORMAT_ID: &str = "simulated-camera:320x240-gray8";

#[cfg(all(target_os = "macos", feature = "real-camera"))]
mod avfoundation;

#[derive(Clone)]
pub struct CameraComponent {
    client: CommandClient<CameraCommand, CommandReceipt>,
    state: Arc<StateCell<Versioned<CameraState>>>,
    events: Arc<EventBus<CameraEvent>>,
    frames: Arc<LatestValueBus<Frame>>,
}

impl CameraComponent {
    pub fn spawn_simulated(component_name: &str) -> Arc<Self> {
        let identity = ComponentIdentity::new("camera", component_name, env!("CARGO_PKG_VERSION"));
        let initial = versioned(identity.clone(), 0, sim_initial_state());
        let state = Arc::new(StateCell::new(initial));
        let events = Arc::new(EventBus::new(256));
        let frames = Arc::new(LatestValueBus::new());
        let (client, inbox) = command_channel(32);
        let component = Arc::new(Self {
            client,
            state: state.clone(),
            events: events.clone(),
            frames: frames.clone(),
        });

        tokio::spawn(run_sim_camera(identity, inbox, state, events, frames));
        component
    }

    #[cfg(all(target_os = "macos", feature = "real-camera"))]
    pub fn spawn_macos(component_name: &str) -> Arc<Self> {
        avfoundation::spawn(component_name)
    }

    #[cfg(not(all(target_os = "macos", feature = "real-camera")))]
    pub fn spawn_macos(component_name: &str) -> Arc<Self> {
        let name = format!("{component_name}-sim-fallback");
        Self::spawn_simulated(&name)
    }
}

#[async_trait]
impl CameraApi for CameraComponent {
    async fn submit(&self, command: CameraCommand) -> Result<CommandReceipt, ApiError> {
        self.client.submit(command).await
    }

    async fn get_state(&self) -> Result<Versioned<CameraState>, ApiError> {
        Ok(self.state.get().await)
    }

    async fn subscribe(&self) -> Result<CameraEventStream, ApiError> {
        Ok(self.events.subscribe())
    }

    async fn subscribe_frames(&self) -> Result<FrameStream, ApiError> {
        Ok(self.frames.subscribe_raw())
    }
}

async fn run_sim_camera(
    identity: ComponentIdentity,
    mut inbox: CommandInbox<CameraCommand, CommandReceipt>,
    state_cell: Arc<StateCell<Versioned<CameraState>>>,
    events: Arc<EventBus<CameraEvent>>,
    frames: Arc<LatestValueBus<Frame>>,
) {
    let sequence = MonotonicCounter::default();
    let mut revision = MonotonicCounter::default();
    let mut state = sim_initial_state();
    let mut frame_id = 0_u64;
    let mut ticker = interval(Duration::from_millis(33));
    let mut last_frame_at = Instant::now();

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if state.lifecycle == CameraLifecycle::Streaming {
                    frame_id = frame_id.saturating_add(1);
                    state.frame_id = frame_id;
                    state.frame_width = WIDTH;
                    state.frame_height = HEIGHT;
                    state.actual_fps = fps_since(&mut last_frame_at);
                    frames.publish(sim_frame(frame_id));
                    publish_event(&identity, &events, &sequence, None, CameraEvent::FrameProduced { frame_id });
                    store_state(&identity, &state_cell, &mut revision, state.clone()).await;
                }
            }
            Some(request) = inbox.recv() => {
                let command = request.command;
                let result = handle_command(
                    &identity,
                    &state_cell,
                    &events,
                    &sequence,
                    &mut revision,
                    &mut state,
                    &command,
                ).await;
                let _ = request.respond_to.send(result);
            }
            else => break,
        }
    }
}

async fn handle_command(
    identity: &ComponentIdentity,
    state_cell: &StateCell<Versioned<CameraState>>,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    revision: &mut MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
) -> Result<CommandReceipt, ApiError> {
    match command.kind {
        CameraCommandKind::RefreshDevices => {
            refresh_sim_devices(identity, events, sequence, state, command)?
        }
        CameraCommandKind::Connect => {
            connect_sim_camera(identity, events, sequence, state, command)?
        }
        CameraCommandKind::StartStream => {
            start_sim_stream(identity, events, sequence, state, command)?
        }
        CameraCommandKind::StopStream => {
            stop_sim_stream(identity, events, sequence, state, command)
        }
        CameraCommandKind::SetRequestedFps { fps } => {
            set_sim_requested_fps(identity, events, sequence, state, command, fps)?;
        }
        CameraCommandKind::SelectDevice { ref device_id } => {
            select_device(state, device_id)?;
            publish_config_event(identity, events, sequence, command, state);
        }
        CameraCommandKind::SelectFormat { ref format_id } => {
            select_format(state, format_id)?;
            publish_config_event(identity, events, sequence, command, state);
        }
    }

    let accepted_revision = store_state(identity, state_cell, revision, state.clone()).await;
    Ok(CommandReceipt {
        command_id: command.command_id,
        operation_id: Some(OperationId::new()),
        accepted_revision,
    })
}

fn refresh_sim_devices(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
) -> Result<(), ApiError> {
    state.available_devices = vec![sim_device()];
    ensure_active_selection(state)?;
    publish_event(
        identity,
        events,
        sequence,
        command.correlation_id,
        CameraEvent::DevicesChanged {
            count: state.available_devices.len(),
        },
    );
    Ok(())
}

fn connect_sim_camera(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
) -> Result<(), ApiError> {
    state.lifecycle = CameraLifecycle::Ready;
    state.permission_status = CameraPermissionStatus::Authorized;
    state.error = None;
    state.frame_width = WIDTH;
    state.frame_height = HEIGHT;
    ensure_active_selection(state)?;
    publish_lifecycle_event(identity, events, sequence, command, state);
    Ok(())
}

fn start_sim_stream(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
) -> Result<(), ApiError> {
    ensure_active_selection(state)?;
    if !matches!(
        state.lifecycle,
        CameraLifecycle::Ready | CameraLifecycle::Streaming
    ) {
        return Err(ApiError::Rejected(
            "camera must be connected before streaming".into(),
        ));
    }
    state.lifecycle = CameraLifecycle::Streaming;
    state.error = None;
    publish_lifecycle_event(identity, events, sequence, command, state);
    Ok(())
}

fn stop_sim_stream(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
) {
    state.lifecycle = CameraLifecycle::Ready;
    state.actual_fps = 0.0;
    publish_lifecycle_event(identity, events, sequence, command, state);
}

fn set_sim_requested_fps(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    state: &mut CameraState,
    command: &CameraCommand,
    fps: f32,
) -> Result<(), ApiError> {
    if !(1.0..=120.0).contains(&fps) {
        return Err(ApiError::InvalidRequest(
            "requested fps must be in the range 1..=120".into(),
        ));
    }
    state.requested_fps = fps;
    publish_event(
        identity,
        events,
        sequence,
        command.correlation_id,
        CameraEvent::RequestedFpsChanged { fps },
    );
    Ok(())
}

fn publish_lifecycle_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    command: &CameraCommand,
    state: &CameraState,
) {
    publish_event(
        identity,
        events,
        sequence,
        command.correlation_id,
        CameraEvent::LifecycleChanged {
            lifecycle: state.lifecycle,
        },
    );
}

fn publish_config_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    command: &CameraCommand,
    state: &CameraState,
) {
    publish_event(
        identity,
        events,
        sequence,
        command.correlation_id,
        CameraEvent::ActiveConfigChanged {
            device_id: state.active_device_id.clone(),
            format_id: state.active_format_id.clone(),
        },
    );
}

fn sim_initial_state() -> CameraState {
    let mut state = CameraState {
        available_devices: vec![sim_device()],
        permission_status: CameraPermissionStatus::Authorized,
        ..CameraState::default()
    };
    let _ = ensure_active_selection(&mut state);
    state
}

fn sim_device() -> CameraDeviceInfo {
    CameraDeviceInfo {
        id: SIM_DEVICE_ID.to_string(),
        display_name: "Simulated Checker Camera".to_string(),
        model_id: Some("vision-lab-sim".to_string()),
        manufacturer: Some("Vision Lab".to_string()),
        position: CameraPosition::Unknown,
        transport: CameraTransport::Virtual,
        is_default: true,
        formats: vec![CameraFormatInfo {
            id: SIM_FORMAT_ID.to_string(),
            width: WIDTH,
            height: HEIGHT,
            pixel_format: PixelFormat::Gray8,
            min_fps: 1.0,
            max_fps: 120.0,
        }],
    }
}

fn ensure_active_selection(state: &mut CameraState) -> Result<(), ApiError> {
    if state.active_device_id.is_none() {
        let device_id = default_device_id(&state.available_devices);
        state.active_device_id = device_id;
    }
    if state.active_format_id.is_none() {
        state.active_format_id = default_format_id(state);
    }
    validate_active_selection(state)
}

fn default_device_id(devices: &[CameraDeviceInfo]) -> Option<String> {
    devices
        .iter()
        .find(|device| device.is_default)
        .or_else(|| devices.first())
        .map(|device| device.id.clone())
}

fn default_format_id(state: &CameraState) -> Option<String> {
    active_device(state)
        .and_then(|device| device.formats.first())
        .map(|format| format.id.clone())
}

fn select_device(state: &mut CameraState, device_id: &str) -> Result<(), ApiError> {
    let device = state
        .available_devices
        .iter()
        .find(|candidate| candidate.id == device_id)
        .ok_or_else(|| ApiError::InvalidRequest(format!("camera device not found: {device_id}")))?;
    state.active_device_id = Some(device.id.clone());
    state.active_format_id = device.formats.first().map(|format| format.id.clone());
    validate_active_selection(state)
}

fn select_format(state: &mut CameraState, format_id: &str) -> Result<(), ApiError> {
    let device = active_device(state).ok_or_else(|| {
        ApiError::InvalidRequest("select a camera device before selecting a format".into())
    })?;
    if !device.formats.iter().any(|format| format.id == format_id) {
        return Err(ApiError::InvalidRequest(format!(
            "camera format not available on active device: {format_id}"
        )));
    }
    state.active_format_id = Some(format_id.to_string());
    Ok(())
}

fn validate_active_selection(state: &CameraState) -> Result<(), ApiError> {
    let Some(device) = active_device(state) else {
        return Err(ApiError::InvalidRequest(
            "no camera device is available".into(),
        ));
    };
    if state.active_format_id.is_none() && !device.formats.is_empty() {
        return Err(ApiError::InvalidRequest(
            "no camera format is selected".into(),
        ));
    }
    if let Some(format_id) = state.active_format_id.as_deref()
        && !device.formats.iter().any(|format| format.id == format_id)
    {
        return Err(ApiError::InvalidRequest(format!(
            "camera format not available on active device: {format_id}"
        )));
    }
    Ok(())
}

fn active_device(state: &CameraState) -> Option<&CameraDeviceInfo> {
    let active_id = state.active_device_id.as_deref()?;
    state
        .available_devices
        .iter()
        .find(|device| device.id == active_id)
}

fn publish_event(
    identity: &ComponentIdentity,
    events: &EventBus<CameraEvent>,
    sequence: &MonotonicCounter,
    correlation_id: Option<comm_core::CorrelationId>,
    payload: CameraEvent,
) {
    events.publish(event(
        identity.clone(),
        sequence.advance(),
        correlation_id,
        payload,
    ));
}

async fn store_state(
    identity: &ComponentIdentity,
    state_cell: &StateCell<Versioned<CameraState>>,
    revision: &mut MonotonicCounter,
    state: CameraState,
) -> u64 {
    let next_revision = revision.advance();
    state_cell
        .set(versioned(identity.clone(), next_revision, state))
        .await;
    next_revision
}

fn fps_since(last_frame_at: &mut Instant) -> f32 {
    let now = Instant::now();
    let elapsed = now.duration_since(*last_frame_at).as_secs_f32();
    *last_frame_at = now;
    if elapsed > 0.0 { 1.0 / elapsed } else { 0.0 }
}

fn sim_frame(frame_id: u64) -> Frame {
    let mut bytes = vec![12_u8; (WIDTH * HEIGHT) as usize];
    let cell = 6_i32;
    let cols = 8_i32;
    let rows = 6_i32;
    let target_w = cols * cell;
    let target_h = rows * cell;
    let x0 = 24_i32 + ((frame_id as i32 * 5) % (WIDTH as i32 - target_w - 48));
    let y0 = 28_i32 + ((frame_id as i32 * 3) % (HEIGHT as i32 - target_h - 56));
    for row in 0..rows {
        for col in 0..cols {
            let value = if (row + col) % 2 == 0 { 232 } else { 28 };
            fill_rect(&mut bytes, x0 + col * cell, y0 + row * cell, cell, value);
        }
    }
    Frame {
        meta: FrameMeta {
            frame_id,
            timestamp: now(),
            width: WIDTH,
            height: HEIGHT,
            stride: WIDTH,
            pixel_format: PixelFormat::Gray8,
        },
        bytes: Bytes::from(bytes),
    }
}

fn fill_rect(bytes: &mut [u8], x0: i32, y0: i32, size: i32, value: u8) {
    for y in y0..(y0 + size) {
        for x in x0..(x0 + size) {
            if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
                bytes[(y as u32 * WIDTH + x as u32) as usize] = value;
            }
        }
    }
}

pub fn real_camera_unavailable_message() -> &'static str {
    "real macOS camera bridge is unavailable; using the simulator fallback"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_device_picks_first_format() {
        let mut state = CameraState {
            available_devices: vec![test_device("a"), test_device("b")],
            ..CameraState::default()
        };
        select_device(&mut state, "b").expect("device should be selectable");
        assert_eq!(state.active_device_id.as_deref(), Some("b"));
        assert_eq!(state.active_format_id.as_deref(), Some("b:640x480"));
    }

    #[test]
    fn select_format_rejects_inactive_device_format() {
        let mut state = CameraState {
            available_devices: vec![test_device("a"), test_device("b")],
            active_device_id: Some("a".to_string()),
            ..CameraState::default()
        };
        let error = select_format(&mut state, "b:640x480").expect_err("format should reject");
        assert!(error.to_string().contains("not available"));
    }

    #[tokio::test]
    async fn simulated_camera_does_not_count_latest_value_replacements_as_drops() {
        let camera = CameraComponent::spawn_simulated("drop-counter-test");
        camera
            .submit(CameraCommand::new(CameraCommandKind::Connect))
            .await
            .unwrap();
        camera
            .submit(CameraCommand::new(CameraCommandKind::StartStream))
            .await
            .unwrap();

        let mut frames = camera.subscribe_frames().await.unwrap();
        for _ in 0..3 {
            frames.changed().await.unwrap();
        }

        let state = camera.get_state().await.unwrap();
        assert!(state.value.frame_id >= 2);
        assert_eq!(state.value.dropped_frames, 0);
    }

    fn test_device(id: &str) -> CameraDeviceInfo {
        CameraDeviceInfo {
            id: id.to_string(),
            display_name: id.to_string(),
            model_id: None,
            manufacturer: None,
            position: CameraPosition::Unknown,
            transport: CameraTransport::Virtual,
            is_default: false,
            formats: vec![CameraFormatInfo {
                id: format!("{id}:640x480"),
                width: 640,
                height: 480,
                pixel_format: PixelFormat::Gray8,
                min_fps: 15.0,
                max_fps: 30.0,
            }],
        }
    }
}
