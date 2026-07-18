import { Slider } from "@/components/ui/slider";

interface ParamSliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  precision?: number;
  onChange: (v: number) => void;
  /** Shown under the label — use for units or a one-line explanation. */
  hint?: string;
  /**
   * Called with true while the user is dragging. The app renders at reduced
   * quality during interaction and refines on release, because a full render
   * takes about a second.
   */
  onInteract?: (active: boolean) => void;
}

/**
 * A labelled slider with a numeric readout, mirroring the Adjust window's
 * "scrollbar + paired edit box" pattern from the Delphi original.
 */
export function ParamSlider({
  label,
  value,
  min,
  max,
  step = 0.01,
  precision = 2,
  onChange,
  hint,
  onInteract,
}: ParamSliderProps) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <label className="text-xs font-medium text-[var(--color-foreground)]">{label}</label>
        <input
          type="number"
          value={Number(value.toFixed(precision))}
          min={min}
          max={max}
          step={step}
          onChange={(e) => {
            const v = Number.parseFloat(e.target.value);
            if (Number.isFinite(v)) onChange(v);
          }}
          className="tabular h-6 w-20 rounded border border-[var(--color-input)] bg-transparent px-1.5 text-right text-xs focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        />
      </div>
      <Slider
        value={[value]}
        min={min}
        max={max}
        step={step}
        onValueChange={([v]) => onChange(v)}
        onPointerDown={() => onInteract?.(true)}
        onPointerUp={() => onInteract?.(false)}
        onKeyDown={() => onInteract?.(true)}
        onKeyUp={() => onInteract?.(false)}
        onBlur={() => onInteract?.(false)}
      />
      {hint && <p className="text-[10px] text-[var(--color-muted-foreground)]">{hint}</p>}
    </div>
  );
}
