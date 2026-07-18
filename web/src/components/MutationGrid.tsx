import { useEffect, useRef } from "react";
import { Dices, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ParamSlider } from "@/components/ParamSlider";
import { TRENDS } from "@/lib/types";

interface MutationGridProps {
  mutants: { size: number; thumbs: ArrayBuffer[] } | null;
  amount: number;
  trend: string;
  generating: boolean;
  onAmount: (v: number) => void;
  onTrend: (v: string) => void;
  onGenerate: () => void;
  onAdopt: (index: number) => void;
}

/**
 * The Mutation window's 3x3 exploration grid.
 *
 * The centre cell is the current flame and the eight around it are mutants.
 * Clicking one adopts it and becomes the new centre, so repeated clicks walk
 * the parameter space — which is how flames like the reference images are
 * actually found, rather than by reasoning about coefficients.
 */
export function MutationGrid({
  mutants,
  amount,
  trend,
  generating,
  onAmount,
  onTrend,
  onGenerate,
  onAdopt,
}: MutationGridProps) {
  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <label className="text-xs font-medium">Trend</label>
        <select
          value={trend}
          onChange={(e) => onTrend(e.target.value)}
          className="h-8 w-full rounded border border-[var(--color-input)] bg-[var(--color-card)] px-2 text-xs focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
        >
          {TRENDS.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </select>
      </div>

      <ParamSlider
        label="Speed"
        value={amount}
        min={0.02}
        max={1}
        step={0.01}
        onChange={onAmount}
        hint="How far each mutant strays from the current flame."
      />

      <Button size="sm" className="w-full" onClick={onGenerate} disabled={generating}>
        {generating ? (
          <RefreshCw className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <Dices className="h-3.5 w-3.5" />
        )}
        {mutants ? "New mutations" : "Generate"}
      </Button>

      {mutants ? (
        <div className="grid grid-cols-3 gap-1">
          {mutants.thumbs.map((buf, i) => (
            <MutantCell
              key={i}
              buffer={buf}
              size={mutants.size}
              isParent={i === 4}
              onClick={() => onAdopt(i)}
            />
          ))}
        </div>
      ) : (
        <p className="py-4 text-center text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
          Generate a grid of variations on the current flame. The centre is what you have now;
          click any neighbour to adopt it and explore from there.
        </p>
      )}

      {mutants && (
        <p className="text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
          Thumbnails render at low quality for speed. Click one to adopt it at full quality.
        </p>
      )}
    </div>
  );
}

function MutantCell({
  buffer,
  size,
  isParent,
  onClick,
}: {
  buffer: ArrayBuffer;
  size: number;
  isParent: boolean;
  onClick: () => void;
}) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    // The buffer was transferred from the worker; if it has already been
    // consumed (a stale render), skip rather than throwing.
    if (buffer.byteLength === 0) return;
    canvas.width = size;
    canvas.height = size;
    canvas
      .getContext("2d")
      ?.putImageData(new ImageData(new Uint8ClampedArray(buffer), size, size), 0, 0);
  }, [buffer, size]);

  return (
    <button
      onClick={onClick}
      disabled={isParent}
      title={isParent ? "Current flame" : "Adopt this mutation"}
      className={`relative aspect-square overflow-hidden rounded transition-all ${
        isParent
          ? "ring-2 ring-[var(--color-primary)]"
          : "ring-1 ring-white/10 hover:ring-2 hover:ring-[var(--color-primary)]"
      }`}
    >
      <canvas ref={ref} className="h-full w-full object-cover" />
      {isParent && (
        <span className="absolute bottom-0.5 left-1 text-[9px] font-medium text-[var(--color-primary)]">
          current
        </span>
      )}
    </button>
  );
}
