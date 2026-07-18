import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Shuffle } from "lucide-react";

const PALETTE_COUNT = 701;

interface PaletteStripProps {
  /** Flat RGB triples for the current palette, 256 entries. */
  rgb: number[] | null;
  index: number;
  onPick: (index: number) => void;
}

/**
 * The gradient tab: a preview strip plus a picker over the 701 palettes
 * carried by the original in `cmapdata.pas`.
 */
export function PaletteStrip({ rgb, index, onPick }: PaletteStripProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [names, setNames] = useState<string[] | null>(null);

  // Names are side-loaded alongside the palette blob.
  useEffect(() => {
    let cancelled = false;
    fetch(`${import.meta.env.BASE_URL}palette-names.json`)
      .then((r) => (r.ok ? r.json() : null))
      .then((n) => {
        if (!cancelled && Array.isArray(n)) setNames(n);
      })
      .catch(() => {
        /* Names are cosmetic — fall back to indices. */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !rgb) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    canvas.width = 256;
    canvas.height = 1;
    const img = ctx.createImageData(256, 1);
    for (let i = 0; i < 256; i++) {
      img.data[i * 4] = rgb[i * 3] ?? 0;
      img.data[i * 4 + 1] = rgb[i * 3 + 1] ?? 0;
      img.data[i * 4 + 2] = rgb[i * 3 + 2] ?? 0;
      img.data[i * 4 + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
  }, [rgb]);

  const label = names?.[index] ?? `palette ${index}`;

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <div className="flex items-baseline justify-between">
          <span className="text-xs font-medium">Gradient</span>
          <span className="tabular text-[10px] text-[var(--color-muted-foreground)]">
            {index + 1} / {PALETTE_COUNT}
          </span>
        </div>
        <canvas
          ref={canvasRef}
          className="h-8 w-full rounded border border-[var(--color-border)]"
          style={{ imageRendering: "pixelated" }}
        />
        <p className="truncate text-[10px] text-[var(--color-muted-foreground)]">{label}</p>
      </div>

      <input
        type="range"
        min={0}
        max={PALETTE_COUNT - 1}
        step={1}
        value={index}
        onChange={(e) => onPick(Number(e.target.value))}
        className="w-full accent-[var(--color-primary)]"
        aria-label="Palette"
      />

      <div className="flex gap-2">
        <Button
          size="sm"
          variant="secondary"
          className="flex-1"
          onClick={() => onPick(Math.floor(Math.random() * PALETTE_COUNT))}
        >
          <Shuffle className="h-3.5 w-3.5" />
          Random
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => onPick(Math.max(0, index - 1))}
          aria-label="Previous palette"
        >
          ‹
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() => onPick(Math.min(PALETTE_COUNT - 1, index + 1))}
          aria-label="Next palette"
        >
          ›
        </Button>
      </div>

      <p className="text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
        All 701 gradients from the original are available. Loading a
        <code className="mx-1 rounded bg-[var(--color-secondary)] px-1">.flame</code>
        file uses the palette embedded in it.
      </p>
    </div>
  );
}
