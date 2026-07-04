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
    CameraApi, CameraCommand, CameraCommandKind, CameraEvent, CameraEventStream, CameraLifecycle,
    CameraState, Frame, FrameMeta, FrameStream, PixelFormat,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

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
        let initial = versioned(identity.clone(), 0, CameraState::default());
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
    let mut state = CameraState::default();
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
                    state.dropped_frames = frames.replaced_count();
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
        CameraCommandKind::Connect => {
            state.lifecycle = CameraLifecycle::Ready;
            state.error = None;
            state.frame_width = WIDTH;
            state.frame_height = HEIGHT;
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
        CameraCommandKind::StartStream => {
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
        CameraCommandKind::StopStream => {
            state.lifecycle = CameraLifecycle::Ready;
            state.actual_fps = 0.0;
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
        CameraCommandKind::SetRequestedFps { fps } => {
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
        }
    }

    let accepted_revision = store_state(identity, state_cell, revision, state.clone()).await;
    Ok(CommandReceipt {
        command_id: command.command_id,
        operation_id: Some(OperationId::new()),
        accepted_revision,
    })
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
    let square = 34_i32;
    let cx = 40_i32 + ((frame_id as i32 * 5) % (WIDTH as i32 - 80));
    let cy = 48_i32 + ((frame_id as i32 * 3) % (HEIGHT as i32 - 96));
    for y in (cy - square / 2)..(cy + square / 2) {
        for x in (cx - square / 2)..(cx + square / 2) {
            if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
                bytes[(y as u32 * WIDTH + x as u32) as usize] =
                    if ((x + y) & 1) == 0 { 235 } else { 178 };
            }
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

pub fn real_camera_unavailable_message() -> &'static str {
    "real macOS camera bridge is not implemented yet; use the default camera-sim feature"
}
