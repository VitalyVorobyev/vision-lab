import { useMemo, useState } from "react";

import type { RingGridTargetConfig, VisionState } from "../../domain/vision";
import { Button } from "../ui/Button";
import { Panel } from "../ui/Panel";
import { Toolbar } from "../ui/Toolbar";

const defaultRingGridTargetConfig: RingGridTargetConfig = {
  rows: 15,
  long_row_cols: 14,
  pitch_mm: 8,
  outer_radius_mm: 4.8,
  inner_radius_mm: 3.2,
  ring_width_mm: 1.152,
};

export function RingGridConfigPanel({
  vision,
  pending,
  onApply,
}: {
  vision?: VisionState;
  pending: (key: string) => boolean;
  onApply: (config: RingGridTargetConfig) => void;
}) {
  const config = vision?.ringgrid_target ?? defaultRingGridTargetConfig;
  const [draft, setDraft] = useState<RingGridTargetConfig>(config);
  const processing = vision?.lifecycle === "Processing";
  const validation = useMemo(() => validate(draft), [draft]);

  function setNumber(key: keyof RingGridTargetConfig, value: string) {
    setDraft((current) => ({ ...current, [key]: Number(value) }));
  }

  return (
    <Panel eyebrow="Calibration target" title="Coded hex RingGrid">
      <div className="grid gap-3">
        <p className="text-xs leading-5 text-muted">
          Configure the printed coded-hex target. Values are millimeters; marker centers remain
          reported in image pixels.
        </p>
        <div className="grid gap-3 sm:grid-cols-2">
          <NumberField
            disabled={processing}
            label="Rows"
            min={1}
            onChange={(value) => setNumber("rows", value)}
            step={1}
            value={draft.rows}
          />
          <NumberField
            disabled={processing}
            label="Long-row columns"
            min={1}
            onChange={(value) => setNumber("long_row_cols", value)}
            step={1}
            value={draft.long_row_cols}
          />
          <NumberField
            disabled={processing}
            label="Pitch"
            min={0.001}
            onChange={(value) => setNumber("pitch_mm", value)}
            step={0.001}
            suffix="mm"
            value={draft.pitch_mm}
          />
          <NumberField
            disabled={processing}
            label="Outer radius"
            min={0.001}
            onChange={(value) => setNumber("outer_radius_mm", value)}
            step={0.001}
            suffix="mm"
            value={draft.outer_radius_mm}
          />
          <NumberField
            disabled={processing}
            label="Inner radius"
            min={0.001}
            onChange={(value) => setNumber("inner_radius_mm", value)}
            step={0.001}
            suffix="mm"
            value={draft.inner_radius_mm}
          />
          <NumberField
            disabled={processing}
            label="Ring width"
            min={0.001}
            onChange={(value) => setNumber("ring_width_mm", value)}
            step={0.001}
            suffix="mm"
            value={draft.ring_width_mm}
          />
        </div>
        {processing ? (
          <p className="text-xs text-warning-text">Stop processing before changing this target.</p>
        ) : null}
        {validation ? <p className="text-xs text-danger-text">{validation}</p> : null}
        <Toolbar>
          <Button
            busy={pending("set-ringgrid-target-config")}
            disabled={processing || validation !== null}
            onClick={() => onApply(draft)}
            variant="primary"
          >
            Apply target
          </Button>
          <Button
            busy={pending("set-ringgrid-target-config")}
            disabled={processing}
            onClick={() => {
              setDraft(defaultRingGridTargetConfig);
              onApply(defaultRingGridTargetConfig);
            }}
            variant="ghost"
          >
            Reset defaults
          </Button>
        </Toolbar>
      </div>
    </Panel>
  );
}

function NumberField({
  disabled,
  label,
  min,
  onChange,
  step,
  suffix,
  value,
}: {
  disabled: boolean;
  label: string;
  min: number;
  onChange: (value: string) => void;
  step: number;
  suffix?: string;
  value: number;
}) {
  return (
    <label className="grid gap-1.5">
      <span className="text-xs font-medium text-muted">{label}</span>
      <span className="relative">
        <input
          className="min-h-9 w-full rounded-md border border-border bg-canvas px-3 pr-10 font-mono text-sm text-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:bg-disabled disabled:text-muted"
          disabled={disabled}
          min={min}
          onChange={(event) => onChange(event.currentTarget.value)}
          step={step}
          type="number"
          value={value}
        />
        {suffix ? (
          <span className="pointer-events-none absolute inset-y-0 right-3 grid place-items-center text-xs text-muted">
            {suffix}
          </span>
        ) : null}
      </span>
    </label>
  );
}

function validate(config: RingGridTargetConfig): string | null {
  const values = Object.values(config);
  if (values.some((value) => !Number.isFinite(value))) return "All target values must be finite.";
  if (!Number.isInteger(config.rows) || config.rows < 1) return "Rows must be a positive integer.";
  if (!Number.isInteger(config.long_row_cols) || config.long_row_cols < 1) {
    return "Long-row columns must be a positive integer.";
  }
  if (config.rows > 1 && config.long_row_cols < 2) {
    return "Multiple rows require at least two long-row columns.";
  }
  if (config.pitch_mm <= 0 || config.outer_radius_mm <= 0 || config.inner_radius_mm <= 0) {
    return "Pitch and radii must be greater than zero.";
  }
  if (config.inner_radius_mm >= config.outer_radius_mm) {
    return "The inner radius must be smaller than the outer radius.";
  }
  if (config.ring_width_mm <= 0 || config.ring_width_mm >= config.outer_radius_mm - config.inner_radius_mm) {
    return "Ring width must leave a positive code band between the two rings.";
  }
  return null;
}
