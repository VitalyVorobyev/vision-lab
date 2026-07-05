import { RefreshCw } from "lucide-react";

import { activeCameraDevice, activeCameraFormat, type CameraState } from "../../domain/camera";
import { Button } from "../ui/Button";
import { MetricGrid } from "../ui/Metric";
import { Panel } from "../ui/Panel";

export function CameraConfigPanel({
  camera,
  pending,
  onRefreshDevices,
  onSelectDevice,
  onSelectFormat,
}: {
  camera?: CameraState;
  pending: (key: string) => boolean;
  onRefreshDevices: () => void;
  onSelectDevice: (deviceId: string) => void;
  onSelectFormat: (formatId: string) => void;
}) {
  const activeDevice = activeCameraDevice(camera);
  const activeFormat = activeCameraFormat(camera);
  const formats = activeDevice?.formats ?? [];

  return (
    <Panel
      action={
        <Button
          busy={pending("refresh-camera-devices")}
          icon={<RefreshCw />}
          onClick={onRefreshDevices}
          variant="ghost"
        >
          Refresh
        </Button>
      }
      eyebrow="Config"
      title="Camera"
    >
      <div className="grid gap-3">
        <MetricGrid
          items={[
            { label: "Permission", value: camera?.permission_status ?? "Unknown" },
            { label: "Devices", value: String(camera?.available_devices.length ?? 0) },
            { label: "Active", value: activeDevice?.display_name ?? "None" },
            {
              label: "Format",
              value: activeFormat ? `${activeFormat.width}x${activeFormat.height}` : "None",
            },
          ]}
        />
        <label className="grid gap-1.5">
          <span className="text-xs font-medium text-muted">Device</span>
          <select
            className="min-h-9 rounded-md border border-border bg-canvas px-3 text-sm text-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
            onChange={(event) => onSelectDevice(event.currentTarget.value)}
            value={camera?.active_device_id ?? ""}
          >
            <option value="">No device</option>
            {(camera?.available_devices ?? []).map((device) => (
              <option key={device.id} value={device.id}>
                {device.display_name}
              </option>
            ))}
          </select>
        </label>
        <label className="grid gap-1.5">
          <span className="text-xs font-medium text-muted">Format</span>
          <select
            className="min-h-9 rounded-md border border-border bg-canvas px-3 text-sm text-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
            disabled={!activeDevice}
            onChange={(event) => onSelectFormat(event.currentTarget.value)}
            value={camera?.active_format_id ?? ""}
          >
            <option value="">No format</option>
            {formats.map((format) => (
              <option key={format.id} value={format.id}>
                {format.width}x{format.height} {format.pixel_format} {format.min_fps.toFixed(0)}-
                {format.max_fps.toFixed(0)} fps
              </option>
            ))}
          </select>
        </label>
        <DeviceDetails camera={camera} />
      </div>
    </Panel>
  );
}

function DeviceDetails({ camera }: { camera?: CameraState }) {
  const activeDevice = activeCameraDevice(camera);
  if (!activeDevice) {
    return <p className="text-sm text-muted">Refresh devices or connect a camera.</p>;
  }
  return (
    <dl className="grid gap-2 text-xs">
      <Detail label="ID" value={activeDevice.id} />
      <Detail label="Model" value={activeDevice.model_id ?? "Unknown"} />
      <Detail label="Manufacturer" value={activeDevice.manufacturer ?? "Unknown"} />
      <Detail label="Position" value={activeDevice.position} />
      <Detail label="Transport" value={activeDevice.transport} />
    </dl>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid min-w-0 grid-cols-[92px_minmax(0,1fr)] gap-2 border border-border bg-surface-muted px-3 py-2">
      <dt className="text-muted">{label}</dt>
      <dd className="truncate font-medium text-text" title={value}>
        {value}
      </dd>
    </div>
  );
}
