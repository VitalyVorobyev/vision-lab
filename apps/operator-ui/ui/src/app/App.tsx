import { useCallback, useRef } from "react";

import type { RectF32 } from "../domain/geometry";
import type { AlgorithmId } from "../domain/vision";
import { useCommandStatus } from "../hooks/useCommandStatus";
import { useFrameCanvas } from "../hooks/useFrameCanvas";
import { useLatestFrame } from "../hooks/useLatestFrame";
import { useRoiInteraction } from "../hooks/useRoiInteraction";
import { useSystemView } from "../hooks/useSystemView";
import {
  captureTemplate,
  connectCamera,
  selectAlgorithm,
  setRequestedFps,
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
    detection: vision?.last_detection?.bbox ?? null,
    frame: latestFrame.frame,
    roi: activeRoi,
  });

  const error = commands.error ?? system.error ?? latestFrame.error;

  return (
    <AppShell
      canvasHandlers={roi.canvasHandlers}
      canvasRef={canvasRef}
      error={error}
      frame={latestFrame.frame}
      onCaptureTemplate={() => void commands.execute("capture-template", captureTemplate)}
      onClearRoi={() => void commands.execute("clear-roi", () => setRoi(null))}
      onConnectCamera={() => void commands.execute("connect-camera", connectCamera)}
      onDismissError={() => {
        commands.clearError();
        system.clearError();
        latestFrame.clearError();
      }}
      onSelectAlgorithm={(algorithm: AlgorithmId) =>
        void commands.execute("select-algorithm", () => selectAlgorithm(algorithm))
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
      view={system.view}
    />
  );
}
