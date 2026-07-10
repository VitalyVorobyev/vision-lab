import { useCallback, useRef, useState } from "react";

import type { RectF32 } from "../domain/geometry";
import {
  defaultOverlayVisibility,
  type OverlayKey,
  type OverlayVisibility,
} from "../domain/overlays";
import type { AlgorithmId, RingGridTargetConfig } from "../domain/vision";
import { useCommandStatus } from "../hooks/useCommandStatus";
import { useFrameCanvas } from "../hooks/useFrameCanvas";
import { useLatestFrame } from "../hooks/useLatestFrame";
import { useRoiInteraction } from "../hooks/useRoiInteraction";
import { useSystemView } from "../hooks/useSystemView";
import {
  captureTemplate,
  connectCamera,
  refreshCameraDevices,
  selectAlgorithm,
  selectCameraDevice,
  selectCameraFormat,
  setRequestedFps,
  setRingGridTargetConfig,
  setRoi,
  startCamera,
  startProcessing,
  startRecording,
  stopCamera,
  stopProcessing,
  stopRecording,
} from "../tauri/commands";
import { AppShell } from "./AppShell";

export function App() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const system = useSystemView();
  const latestFrame = useLatestFrame();
  const commands = useCommandStatus();
  const vision = system.view?.vision.value;
  const [overlays, setOverlays] = useState<OverlayVisibility>(defaultOverlayVisibility);

  const commitRoi = useCallback(
    (roi: RectF32) => {
      void commands.execute("set-roi", () => setRoi(roi));
    },
    [commands],
  );

  const roi = useRoiInteraction(latestFrame.frame, commitRoi);
  const activeRoi = roi.pendingRoi ?? vision?.roi ?? null;

  useFrameCanvas({
    canvasRef,
    detection: vision?.last_detection ?? null,
    frame: latestFrame.frame,
    overlays,
    roi: activeRoi,
  });

  const error = commands.error ?? system.error ?? latestFrame.error;
  const runChess = useCallback(async () => {
    await refreshCameraDevices();
    await connectCamera();
    await startCamera();
    await selectAlgorithm("ChessCorners");
    await startProcessing();
  }, []);

  return (
    <AppShell
      canvasHandlers={roi.canvasHandlers}
      canvasRef={canvasRef}
      error={error}
      frame={latestFrame.frame}
      onToggleOverlay={(key: OverlayKey) =>
        setOverlays((current) => ({ ...current, [key]: !current[key] }))
      }
      onCaptureTemplate={() => void commands.execute("capture-template", captureTemplate)}
      onClearRoi={() => void commands.execute("clear-roi", () => setRoi(null))}
      onConnectCamera={() => void commands.execute("connect-camera", connectCamera)}
      onDismissError={() => {
        commands.clearError();
        system.clearError();
        latestFrame.clearError();
      }}
      onRunChess={() => void commands.execute("run-chess", runChess)}
      onRefreshCameraDevices={() =>
        void commands.execute("refresh-camera-devices", refreshCameraDevices)
      }
      onSelectAlgorithm={(algorithm: AlgorithmId) =>
        void commands.execute("select-algorithm", () => selectAlgorithm(algorithm))
      }
      onSetRingGridTargetConfig={(config: RingGridTargetConfig) =>
        void commands.execute("set-ringgrid-target-config", () => setRingGridTargetConfig(config))
      }
      onSelectCameraDevice={(deviceId: string) =>
        void commands.execute("select-camera-device", () => selectCameraDevice(deviceId))
      }
      onSelectCameraFormat={(formatId: string) =>
        void commands.execute("select-camera-format", () => selectCameraFormat(formatId))
      }
      onSetRequestedFps={(fps: number) =>
        void commands.execute("set-requested-fps", () => setRequestedFps(fps))
      }
      onStartCamera={() => void commands.execute("start-camera", startCamera)}
      onStartProcessing={() => void commands.execute("start-processing", startProcessing)}
      onStartRecording={() => void commands.execute("start-recording", () => startRecording(8))}
      onStopCamera={() => void commands.execute("stop-camera", stopCamera)}
      onStopProcessing={() => void commands.execute("stop-processing", stopProcessing)}
      onStopRecording={() => void commands.execute("stop-recording", stopRecording)}
      pending={commands.isPending}
      pendingRoi={roi.pendingRoi}
      overlays={overlays}
      view={system.view}
    />
  );
}
