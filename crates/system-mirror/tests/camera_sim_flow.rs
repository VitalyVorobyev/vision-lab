use camera_mac::CameraComponent;
use std::{sync::Arc, time::Duration};
use system_mirror::SystemMirror;
use tempfile::tempdir;
use tokio::time::sleep;
use vision_contracts::{
    CameraApi, CameraCommand, CameraCommandKind, RecorderApi, RecorderCommand, RecorderCommandKind,
    RectF32, VisionApi, VisionCommand, VisionCommandKind,
};
use vision_processing::VisionComponent;

#[tokio::test]
async fn camera_sim_command_event_state_detection_flow() {
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
    let mirror = SystemMirror::spawn(camera_api.clone(), vision_api.clone(), recorder_api.clone())
        .await
        .unwrap();

    camera_api
        .submit(CameraCommand::new(CameraCommandKind::Connect))
        .await
        .unwrap();
    camera_api
        .submit(CameraCommand::new(CameraCommandKind::StartStream))
        .await
        .unwrap();

    let mut frames = camera_api.subscribe_frames().await.unwrap();
    let frame = loop {
        frames.changed().await.unwrap();
        if let Some(frame) = frames.borrow().clone() {
            break frame;
        }
    };
    let roi = bright_bbox(&frame).expect("sim frame should contain a textured target");
    vision_api
        .submit(VisionCommand::new(VisionCommandKind::SetRoi {
            roi: Some(roi),
        }))
        .await
        .unwrap();
    vision_api
        .submit(VisionCommand::new(VisionCommandKind::CaptureTemplate))
        .await
        .unwrap();
    vision_api
        .submit(VisionCommand::new(VisionCommandKind::StartProcessing))
        .await
        .unwrap();
    recorder_api
        .submit(RecorderCommand::new(RecorderCommandKind::StartRecording {
            max_fps: 20.0,
        }))
        .await
        .unwrap();

    let mut detected = false;
    for _ in 0..30 {
        sleep(Duration::from_millis(50)).await;
        let view = mirror.current().await;
        if view.vision.value.last_detection.is_some() && view.recorder.value.recorded_frames > 0 {
            detected = true;
            break;
        }
    }
    assert!(
        detected,
        "mirror should observe detection and recorder progress"
    );

    recorder_api
        .submit(RecorderCommand::new(RecorderCommandKind::StopRecording))
        .await
        .unwrap();
    vision_api
        .submit(VisionCommand::new(VisionCommandKind::StopProcessing))
        .await
        .unwrap();
    camera_api
        .submit(CameraCommand::new(CameraCommandKind::StopStream))
        .await
        .unwrap();
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
