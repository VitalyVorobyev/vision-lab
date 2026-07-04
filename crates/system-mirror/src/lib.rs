//! Host-side normalized system view for UI consumers.

use comm_core::{ApiError, ComponentIdentity, EventEnvelope, Versioned};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, broadcast, mpsc};
use vision_contracts::{
    CameraApi, CameraEvent, CameraState, RecorderApi, RecorderEvent, RecorderState, VisionApi,
    VisionEvent, VisionState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemView {
    pub camera: Versioned<CameraState>,
    pub vision: Versioned<VisionState>,
    pub recorder: Versioned<RecorderState>,
    pub recent_events: Vec<EventSummary>,
    pub resync_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub source: ComponentIdentity,
    pub sequence: u64,
    pub event_id: String,
    pub correlation_id: Option<String>,
    pub summary: String,
}

#[derive(Clone)]
pub struct SystemMirror {
    state: Arc<RwLock<SystemView>>,
    tx: broadcast::Sender<SystemView>,
}

impl SystemMirror {
    pub async fn spawn(
        camera: Arc<dyn CameraApi>,
        vision: Arc<dyn VisionApi>,
        recorder: Arc<dyn RecorderApi>,
    ) -> Result<Arc<Self>, ApiError> {
        let initial = SystemView {
            camera: camera.get_state().await?,
            vision: vision.get_state().await?,
            recorder: recorder.get_state().await?,
            recent_events: Vec::new(),
            resync_count: 0,
        };
        let state = Arc::new(RwLock::new(initial.clone()));
        let (tx, _) = broadcast::channel(64);
        let (update_tx, update_rx) = mpsc::channel(256);
        let mirror = Arc::new(Self {
            state: state.clone(),
            tx: tx.clone(),
        });

        spawn_camera_subscription(camera.clone(), update_tx.clone()).await?;
        spawn_vision_subscription(vision.clone(), update_tx.clone()).await?;
        spawn_recorder_subscription(recorder.clone(), update_tx.clone()).await?;
        tokio::spawn(run_mirror(camera, vision, recorder, state, tx, update_rx));
        Ok(mirror)
    }

    pub async fn current(&self) -> SystemView {
        self.state.read().await.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemView> {
        self.tx.subscribe()
    }
}

enum MirrorUpdate {
    Camera(Result<EventEnvelope<CameraEvent>, StreamIssue>),
    Vision(Result<EventEnvelope<VisionEvent>, StreamIssue>),
    Recorder(Result<EventEnvelope<RecorderEvent>, StreamIssue>),
}

#[derive(Debug)]
enum StreamIssue {
    Lagged,
    Closed,
}

async fn spawn_camera_subscription(
    camera: Arc<dyn CameraApi>,
    tx: mpsc::Sender<MirrorUpdate>,
) -> Result<(), ApiError> {
    let mut events = camera.subscribe().await?;
    tokio::spawn(async move {
        loop {
            let update = match events.recv().await {
                Ok(event) => MirrorUpdate::Camera(Ok(event)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    MirrorUpdate::Camera(Err(StreamIssue::Lagged))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    MirrorUpdate::Camera(Err(StreamIssue::Closed))
                }
            };
            let closed = matches!(update, MirrorUpdate::Camera(Err(StreamIssue::Closed)));
            if tx.send(update).await.is_err() || closed {
                break;
            }
        }
    });
    Ok(())
}

async fn spawn_vision_subscription(
    vision: Arc<dyn VisionApi>,
    tx: mpsc::Sender<MirrorUpdate>,
) -> Result<(), ApiError> {
    let mut events = vision.subscribe().await?;
    tokio::spawn(async move {
        loop {
            let update = match events.recv().await {
                Ok(event) => MirrorUpdate::Vision(Ok(event)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    MirrorUpdate::Vision(Err(StreamIssue::Lagged))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    MirrorUpdate::Vision(Err(StreamIssue::Closed))
                }
            };
            let closed = matches!(update, MirrorUpdate::Vision(Err(StreamIssue::Closed)));
            if tx.send(update).await.is_err() || closed {
                break;
            }
        }
    });
    Ok(())
}

async fn spawn_recorder_subscription(
    recorder: Arc<dyn RecorderApi>,
    tx: mpsc::Sender<MirrorUpdate>,
) -> Result<(), ApiError> {
    let mut events = recorder.subscribe().await?;
    tokio::spawn(async move {
        loop {
            let update = match events.recv().await {
                Ok(event) => MirrorUpdate::Recorder(Ok(event)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    MirrorUpdate::Recorder(Err(StreamIssue::Lagged))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    MirrorUpdate::Recorder(Err(StreamIssue::Closed))
                }
            };
            let closed = matches!(update, MirrorUpdate::Recorder(Err(StreamIssue::Closed)));
            if tx.send(update).await.is_err() || closed {
                break;
            }
        }
    });
    Ok(())
}

async fn run_mirror(
    camera: Arc<dyn CameraApi>,
    vision: Arc<dyn VisionApi>,
    recorder: Arc<dyn RecorderApi>,
    state: Arc<RwLock<SystemView>>,
    tx: broadcast::Sender<SystemView>,
    mut rx: mpsc::Receiver<MirrorUpdate>,
) {
    let mut tracker = SequenceTracker::default();
    while let Some(update) = rx.recv().await {
        let mut needs_resync = false;
        let mut refresh_camera = false;
        let mut refresh_vision = false;
        let mut refresh_recorder = false;
        let mut summary = None;
        match update {
            MirrorUpdate::Camera(Ok(event)) => {
                needs_resync |= tracker.observe(&event.source, event.sequence);
                refresh_camera = true;
                summary = Some(summarize_camera(event));
            }
            MirrorUpdate::Vision(Ok(event)) => {
                needs_resync |= tracker.observe(&event.source, event.sequence);
                refresh_vision = true;
                summary = Some(summarize_vision(event));
            }
            MirrorUpdate::Recorder(Ok(event)) => {
                needs_resync |= tracker.observe(&event.source, event.sequence);
                refresh_recorder = true;
                summary = Some(summarize_recorder(event));
            }
            MirrorUpdate::Camera(Err(StreamIssue::Lagged))
            | MirrorUpdate::Vision(Err(StreamIssue::Lagged))
            | MirrorUpdate::Recorder(Err(StreamIssue::Lagged)) => {
                needs_resync = true;
                refresh_camera = true;
                refresh_vision = true;
                refresh_recorder = true;
            }
            MirrorUpdate::Camera(Err(StreamIssue::Closed))
            | MirrorUpdate::Vision(Err(StreamIssue::Closed))
            | MirrorUpdate::Recorder(Err(StreamIssue::Closed)) => continue,
        }

        {
            let mut view = state.write().await;
            if (needs_resync || refresh_camera)
                && let Ok(camera_state) = camera.get_state().await
            {
                view.camera = camera_state;
            }
            if (needs_resync || refresh_vision)
                && let Ok(vision_state) = vision.get_state().await
            {
                view.vision = vision_state;
            }
            if (needs_resync || refresh_recorder)
                && let Ok(recorder_state) = recorder.get_state().await
            {
                view.recorder = recorder_state;
            }
            if needs_resync {
                view.resync_count = view.resync_count.saturating_add(1);
            }
            if let Some(summary) = summary {
                view.recent_events.push(summary);
                if view.recent_events.len() > 80 {
                    let excess = view.recent_events.len() - 80;
                    view.recent_events.drain(0..excess);
                }
            }
            let _ = tx.send(view.clone());
        }
    }
}

#[derive(Default)]
struct SequenceTracker {
    last_by_source: HashMap<String, u64>,
}

impl SequenceTracker {
    fn observe(&mut self, source: &ComponentIdentity, sequence: u64) -> bool {
        let key = format!(
            "{}:{}:{}",
            source.component.component_type, source.component.component_name, source.instance_id.0
        );
        let gap = self
            .last_by_source
            .get(&key)
            .is_some_and(|last| sequence != last.saturating_add(1));
        self.last_by_source.insert(key, sequence);
        gap
    }
}

fn summarize_camera(event: EventEnvelope<CameraEvent>) -> EventSummary {
    summarize(event, |payload| match payload {
        CameraEvent::LifecycleChanged { lifecycle } => format!("camera lifecycle: {lifecycle:?}"),
        CameraEvent::RequestedFpsChanged { fps } => format!("camera requested fps: {fps:.1}"),
        CameraEvent::FrameProduced { frame_id } => format!("frame produced: {frame_id}"),
        CameraEvent::DroppedFramesChanged { dropped_frames } => {
            format!("camera dropped frames: {dropped_frames}")
        }
        CameraEvent::Error { message } => format!("camera error: {message}"),
    })
}

fn summarize_vision(event: EventEnvelope<VisionEvent>) -> EventSummary {
    summarize(event, |payload| match payload {
        VisionEvent::LifecycleChanged { lifecycle } => format!("vision lifecycle: {lifecycle:?}"),
        VisionEvent::AlgorithmSelected { algorithm } => {
            format!("algorithm selected: {algorithm:?}")
        }
        VisionEvent::RoiChanged { roi } => format!("ROI changed: {roi:?}"),
        VisionEvent::TemplateCaptured { width, height } => {
            format!("template captured: {width}x{height}")
        }
        VisionEvent::DetectionProduced { detection } => {
            format!(
                "detection {} conf {:.3}",
                detection.frame_id, detection.confidence
            )
        }
        VisionEvent::MetricsUpdated => "vision metrics updated".to_string(),
        VisionEvent::Error { message } => format!("vision error: {message}"),
    })
}

fn summarize_recorder(event: EventEnvelope<RecorderEvent>) -> EventSummary {
    summarize(event, |payload| match payload {
        RecorderEvent::LifecycleChanged { lifecycle } => {
            format!("recorder lifecycle: {lifecycle:?}")
        }
        RecorderEvent::SessionStarted { path } => format!("recording started: {path}"),
        RecorderEvent::SessionStopped { path } => format!("recording stopped: {path}"),
        RecorderEvent::FrameRecorded { frame_id } => format!("frame recorded: {frame_id}"),
        RecorderEvent::DetectionRecorded { frame_id } => {
            format!("detection recorded for frame: {frame_id}")
        }
        RecorderEvent::Error { message } => format!("recorder error: {message}"),
    })
}

fn summarize<T>(event: EventEnvelope<T>, f: impl FnOnce(T) -> String) -> EventSummary {
    EventSummary {
        source: event.source,
        sequence: event.sequence,
        event_id: event.event_id.0.to_string(),
        correlation_id: event.correlation_id.map(|id| id.0.to_string()),
        summary: f(event.payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_detects_sequence_gap() {
        let source = ComponentIdentity::new("vision", "main", "0");
        let mut tracker = SequenceTracker::default();
        assert!(!tracker.observe(&source, 1));
        assert!(!tracker.observe(&source, 2));
        assert!(tracker.observe(&source, 4));
        assert!(!tracker.observe(&source, 5));
    }

    #[test]
    fn tracker_detects_restart_sequence_reset() {
        let source = ComponentIdentity::new("vision", "main", "0");
        let mut tracker = SequenceTracker::default();
        assert!(!tracker.observe(&source, 7));
        assert!(tracker.observe(&source, 1));
    }
}
