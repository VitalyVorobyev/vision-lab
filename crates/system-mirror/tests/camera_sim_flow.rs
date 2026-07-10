use camera_mac::CameraComponent;
use std::{sync::Arc, time::Duration};
use system_mirror::SystemMirror;
use tempfile::{TempDir, tempdir};
use tokio::time::sleep;
use vision_contracts::{
    AlgorithmId, CameraApi, CameraCommand, CameraCommandKind, RecorderApi, RecorderCommand,
    RecorderCommandKind, RectF32, RingGridTargetConfig, VisionApi, VisionCommand,
    VisionCommandKind,
};
use vision_processing::VisionComponent;

struct TestStack {
    camera: Arc<dyn CameraApi>,
    vision: Arc<dyn VisionApi>,
    recorder: Arc<dyn RecorderApi>,
    mirror: Arc<SystemMirror>,
    _temp: TempDir,
}

impl TestStack {
    async fn spawn() -> Self {
        let camera = CameraComponent::spawn_simulated("test-camera");
        let camera_api: Arc<dyn CameraApi> = camera;
        let vision = VisionComponent::spawn("test-vision", camera_api.clone())
            .await
            .unwrap();
        let vision_api: Arc<dyn VisionApi> = vision;
        let temp = tempdir().unwrap();
        let recorder = recorder::RecorderComponent::spawn(
            "test-recorder",
            camera_api.clone(),
            vision_api.clone(),
            temp.path().to_path_buf(),
        )
        .await
        .unwrap();
        let recorder_api: Arc<dyn RecorderApi> = recorder;
        let mirror =
            SystemMirror::spawn(camera_api.clone(), vision_api.clone(), recorder_api.clone())
                .await
                .unwrap();
        Self {
            camera: camera_api,
            vision: vision_api,
            recorder: recorder_api,
            mirror,
            _temp: temp,
        }
    }

    async fn start_camera(&self) {
        self.camera
            .submit(CameraCommand::new(CameraCommandKind::Connect))
            .await
            .unwrap();
        self.camera
            .submit(CameraCommand::new(CameraCommandKind::StartStream))
            .await
            .unwrap();
    }

    async fn next_frame(&self) -> Arc<vision_contracts::Frame> {
        let mut frames = self.camera.subscribe_frames().await.unwrap();
        loop {
            frames.changed().await.unwrap();
            if let Some(frame) = frames.borrow().clone() {
                break frame;
            }
        }
    }
}

#[tokio::test]
async fn camera_sim_command_event_state_detection_flow() {
    let stack = TestStack::spawn().await;
    stack.start_camera().await;

    let frame = stack.next_frame().await;
    let roi = template_roi(&frame).expect("sim frame should contain a textured target");
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::SelectAlgorithm {
            algorithm: AlgorithmId::TemplateNcc,
        }))
        .await
        .unwrap();
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::SetRoi {
            roi: Some(roi),
        }))
        .await
        .unwrap();
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::CaptureTemplate))
        .await
        .unwrap();

    start_processing_and_recording(&stack).await;
    assert_template_progress(&stack).await;
    stop_all(&stack).await;
}

#[tokio::test]
async fn chess_online_flow_produces_overlay_points() {
    let stack = TestStack::spawn().await;
    stack.start_camera().await;
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::SelectAlgorithm {
            algorithm: AlgorithmId::ChessCorners,
        }))
        .await
        .unwrap();
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::StartProcessing))
        .await
        .unwrap();

    let detected = wait_until(&stack, |view| {
        view.vision
            .value
            .last_detection
            .as_ref()
            .is_some_and(|detection| {
                detection.method == AlgorithmId::ChessCorners && !detection.points.is_empty()
            })
    })
    .await;
    assert!(detected, "mirror should observe ChESS detection points");
    stop_all(&stack).await;
}

#[tokio::test]
async fn ringgrid_target_config_reaches_mirror_and_recording_manifest() {
    let stack = TestStack::spawn().await;
    let config = RingGridTargetConfig {
        rows: 3,
        long_row_cols: 3,
        pitch_mm: 8.0,
        outer_radius_mm: 2.4,
        inner_radius_mm: 1.4,
        ring_width_mm: 0.5,
    };
    stack
        .vision
        .submit(VisionCommand::new(
            VisionCommandKind::SetRingGridTargetConfig {
                config: config.clone(),
            },
        ))
        .await
        .unwrap();

    let mirrored = wait_until(&stack, |view| view.vision.value.ringgrid_target == config).await;
    assert!(
        mirrored,
        "mirror should expose the configured RingGrid target"
    );

    stack
        .recorder
        .submit(RecorderCommand::new(RecorderCommandKind::StartRecording {
            max_fps: 20.0,
        }))
        .await
        .unwrap();
    let session_path = stack
        .recorder
        .get_state()
        .await
        .unwrap()
        .value
        .session_path
        .expect("recording should have a session path");
    let manifest = std::fs::read_to_string(format!("{session_path}/manifest.json")).unwrap();
    assert!(manifest.contains("ringgrid_target"));
    assert!(manifest.contains("\"rows\": 3"));

    stop_all(&stack).await;
}

#[tokio::test]
async fn ringgrid_target_config_is_rejected_without_state_change_while_processing() {
    let stack = TestStack::spawn().await;
    stack.start_camera().await;
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::StartProcessing))
        .await
        .unwrap();

    let initial = stack.mirror.current().await.vision.value.ringgrid_target;
    let result = stack
        .vision
        .submit(VisionCommand::new(
            VisionCommandKind::SetRingGridTargetConfig {
                config: RingGridTargetConfig {
                    rows: 3,
                    long_row_cols: 3,
                    pitch_mm: 8.0,
                    outer_radius_mm: 2.4,
                    inner_radius_mm: 1.4,
                    ring_width_mm: 0.5,
                },
            },
        ))
        .await;
    assert!(result.is_err());
    assert_eq!(
        stack.mirror.current().await.vision.value.ringgrid_target,
        initial
    );

    stop_all(&stack).await;
}

async fn start_processing_and_recording(stack: &TestStack) {
    stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::StartProcessing))
        .await
        .unwrap();
    stack
        .recorder
        .submit(RecorderCommand::new(RecorderCommandKind::StartRecording {
            max_fps: 20.0,
        }))
        .await
        .unwrap();
}

async fn assert_template_progress(stack: &TestStack) {
    let detected = wait_until(stack, |view| {
        view.vision.value.last_detection.is_some() && view.recorder.value.recorded_frames > 0
    })
    .await;
    assert!(
        detected,
        "mirror should observe detection and recorder progress"
    );
}

async fn wait_until(
    stack: &TestStack,
    matches: impl Fn(&system_mirror::SystemView) -> bool,
) -> bool {
    for _ in 0..30 {
        sleep(Duration::from_millis(50)).await;
        let view = stack.mirror.current().await;
        if matches(&view) {
            return true;
        }
    }
    false
}

async fn stop_all(stack: &TestStack) {
    let _ = stack
        .recorder
        .submit(RecorderCommand::new(RecorderCommandKind::StopRecording))
        .await;
    let _ = stack
        .vision
        .submit(VisionCommand::new(VisionCommandKind::StopProcessing))
        .await;
    let _ = stack
        .camera
        .submit(CameraCommand::new(CameraCommandKind::StopStream))
        .await;
}

fn bright_bbox(frame: &vision_contracts::Frame) -> Option<RectF32> {
    let mut min_x = frame.meta.width;
    let mut min_y = frame.meta.height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..frame.meta.height {
        for x in 0..frame.meta.width {
            let value = frame.bytes[(y * frame.meta.stride + x) as usize];
            if value > 100 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (max_x > min_x && max_y > min_y).then_some(RectF32 {
        x: min_x as f32,
        y: min_y as f32,
        width: (max_x - min_x + 1) as f32,
        height: (max_y - min_y + 1) as f32,
    })
}

fn template_roi(frame: &vision_contracts::Frame) -> Option<RectF32> {
    let target = bright_bbox(frame)?;
    Some(RectF32 {
        x: target.x,
        y: target.y,
        width: 12.0,
        height: 12.0,
    })
}
