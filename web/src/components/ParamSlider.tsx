import { useEffect, useRef, useState } from "react";
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
  // The text box drafts locally and commits on blur/Enter. Committing every
  // keystroke fires a full-quality render per digit ("500" renders at 5 and
  // 50 first) and makes it impossible to type through an out-of-range prefix.
  const [draft, setDraft] = useState<string | null>(null);
  const lastValue = useRef(value);
  useEffect(() => {
    // External change while not editing: drop any stale draft.
    if (draft !== null && value !== lastValue.current) setDraft(null);
    lastValue.current = value;
  }, [value, draft]);

  const commitDraft = () => {
    if (draft === null) return;
    const v = Number.parseFloat(draft);
    setDraft(null);
    if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)));
  };

  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <label className="text-xs font-medium text-[var(--color-foreground)]">{label}</label>
        <input
          type="number"
          value={draft ?? String(Number(value.toFixed(precision)))}
          min={min}
          max={max}
          step={step}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitDraft}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitDraft();
            else if (e.key === "Escape") setDraft(null);
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
