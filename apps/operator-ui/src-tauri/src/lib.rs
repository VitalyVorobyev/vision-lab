use base64::{Engine, engine::general_purpose};
use camera_mac::CameraComponent;
use comm_core::{ApiError, CommandReceipt};
use recorder::RecorderComponent;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use system_mirror::{SystemMirror, SystemView};
use tauri::{Emitter, Manager, State};
use tokio::sync::RwLock;
use vision_contracts::{
    AlgorithmId, CameraApi, CameraCommand, CameraCommandKind, FrameMeta, RecorderApi,
    RecorderCommand, RecorderCommandKind, RectF32, VisionApi, VisionCommand, VisionCommandKind,
};
use vision_processing::VisionComponent;

struct AppRuntime {
    camera: Arc<dyn CameraApi>,
    vision: Arc<dyn VisionApi>,
    recorder: Arc<dyn RecorderApi>,
    mirror: Arc<SystemMirror>,
    latest_frame: Arc<RwLock<Option<FramePayload>>>,
}

#[derive(Debug, Clone, Serialize)]
struct FramePayload {
    meta: FrameMeta,
    data_base64: String,
}

#[tauri::command]
async fn system_view(runtime: State<'_, AppRuntime>) -> Result<SystemView, String> {
    Ok(runtime.mirror.current().await)
}

#[tauri::command]
async fn latest_frame(runtime: State<'_, AppRuntime>) -> Result<Option<FramePayload>, String> {
    Ok(runtime.latest_frame.read().await.clone())
}

#[tauri::command]
async fn connect_camera(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_camera(&runtime, CameraCommandKind::Connect).await
}

#[tauri::command]
async fn start_camera(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_camera(&runtime, CameraCommandKind::StartStream).await
}

#[tauri::command]
async fn stop_camera(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_camera(&runtime, CameraCommandKind::StopStream).await
}

#[tauri::command]
async fn set_requested_fps(
    runtime: State<'_, AppRuntime>,
    fps: f32,
) -> Result<CommandReceipt, String> {
    submit_camera(&runtime, CameraCommandKind::SetRequestedFps { fps }).await
}

#[tauri::command]
async fn select_algorithm(
    runtime: State<'_, AppRuntime>,
    algorithm: AlgorithmId,
) -> Result<CommandReceipt, String> {
    submit_vision(&runtime, VisionCommandKind::SelectAlgorithm { algorithm }).await
}

#[tauri::command]
async fn set_roi(
    runtime: State<'_, AppRuntime>,
    roi: Option<RectF32>,
) -> Result<CommandReceipt, String> {
    submit_vision(&runtime, VisionCommandKind::SetRoi { roi }).await
}

#[tauri::command]
async fn capture_template(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_vision(&runtime, VisionCommandKind::CaptureTemplate).await
}

#[tauri::command]
async fn start_processing(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_vision(&runtime, VisionCommandKind::StartProcessing).await
}

#[tauri::command]
async fn stop_processing(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_vision(&runtime, VisionCommandKind::StopProcessing).await
}

#[tauri::command]
async fn start_recording(
    runtime: State<'_, AppRuntime>,
    max_fps: f32,
) -> Result<CommandReceipt, String> {
    submit_recorder(&runtime, RecorderCommandKind::StartRecording { max_fps }).await
}

#[tauri::command]
async fn stop_recording(runtime: State<'_, AppRuntime>) -> Result<CommandReceipt, String> {
    submit_recorder(&runtime, RecorderCommandKind::StopRecording).await
}

async fn submit_camera(
    runtime: &AppRuntime,
    kind: CameraCommandKind,
) -> Result<CommandReceipt, String> {
    runtime
        .camera
        .submit(CameraCommand::new(kind))
        .await
        .map_err(error_message)
}

async fn submit_vision(
    runtime: &AppRuntime,
    kind: VisionCommandKind,
) -> Result<CommandReceipt, String> {
    runtime
        .vision
        .submit(VisionCommand::new(kind))
        .await
        .map_err(error_message)
}

async fn submit_recorder(
    runtime: &AppRuntime,
    kind: RecorderCommandKind,
) -> Result<CommandReceipt, String> {
    runtime
        .recorder
        .submit(RecorderCommand::new(kind))
        .await
        .map_err(error_message)
}

fn error_message(error: ApiError) -> String {
    error.to_string()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let runtime = tauri::async_runtime::block_on(async {
                let camera = CameraComponent::spawn_simulated("macbook-camera-sim");
                let camera_api: Arc<dyn CameraApi> = camera;
                let vision = VisionComponent::spawn("main", camera_api.clone()).await?;
                let vision_api: Arc<dyn VisionApi> = vision;
                let recorder_base = default_session_dir();
                let recorder = RecorderComponent::spawn(
                    "main",
                    camera_api.clone(),
                    vision_api.clone(),
                    recorder_base,
                )
                .await?;
                let recorder_api: Arc<dyn RecorderApi> = recorder;
                let mirror = SystemMirror::spawn(
                    camera_api.clone(),
                    vision_api.clone(),
                    recorder_api.clone(),
                )
                .await?;
                Ok::<_, ApiError>(AppRuntime {
                    camera: camera_api,
                    vision: vision_api,
                    recorder: recorder_api,
                    mirror,
                    latest_frame: Arc::new(RwLock::new(None)),
                })
            })
            .map_err(|error| error.to_string())?;

            spawn_system_view_emitter(app_handle.clone(), runtime.mirror.clone());
            spawn_frame_emitter(
                app_handle,
                runtime.camera.clone(),
                runtime.latest_frame.clone(),
            );
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system_view,
            latest_frame,
            connect_camera,
            start_camera,
            stop_camera,
            set_requested_fps,
            select_algorithm,
            set_roi,
            capture_template,
            start_processing,
            stop_processing,
            start_recording,
            stop_recording,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run operator-ui");
}

fn spawn_system_view_emitter(app: tauri::AppHandle, mirror: Arc<SystemMirror>) {
    let mut updates = mirror.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(view) = updates.recv().await {
            let _ = app.emit("system-view", view);
        }
    });
}

fn spawn_frame_emitter(
    app: tauri::AppHandle,
    camera: Arc<dyn CameraApi>,
    latest_frame: Arc<RwLock<Option<FramePayload>>>,
) {
    tauri::async_runtime::spawn(async move {
        let Ok(mut frames) = camera.subscribe_frames().await else {
            return;
        };
        while frames.changed().await.is_ok() {
            let Some(frame) = frames.borrow().clone() else {
                continue;
            };
            let payload = FramePayload {
                meta: frame.meta.clone(),
                data_base64: general_purpose::STANDARD.encode(&frame.bytes),
            };
            *latest_frame.write().await = Some(payload.clone());
            let _ = app.emit("frame", payload);
        }
    });
}

fn default_session_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("recordings")
}
