import { useCallback, useEffect, useRef, useState } from "react";
import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";

const CHANNELS = [
  { label: "Overall", colour: "#e5e5e5" },
  { label: "Red", colour: "#ff6b6b" },
  { label: "Green", colour: "#6bff9c" },
  { label: "Blue", colour: "#6bb6ff" },
];

interface CurvesEditorProps {
  /** The flat 48-value layout: 4 channels x 4 points x (x, y, weight). */
  curves: number[] | null;
  onPoint: (channel: number, index: number, x: number, y: number) => void;
  onReset: (channel: number) => void;
  onInteract: (active: boolean) => void;
}

/**
 * The Curves window: four rational-cubic Bézier tone curves.
 *
 * A caveat worth knowing while dragging: the original samples the curve
 * uniformly in the Bézier *parameter* (`BezierFunc(i/256, ...)`), not in x. So
 * a control point's x-coordinate does not affect the rendered result at all —
 * only its y does. The x is still stored and drawn, because the original draws
 * and serialises it, but moving a point sideways changes nothing.
 */
export function CurvesEditor({ curves, onPoint, onReset, onInteract }: CurvesEditorProps) {
  const [channel, setChannel] = useState(0);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dragging = useRef<number | null>(null);
  const SIZE = 240;

  /** Control points of the active channel as [x, y] pairs. */
  const points = useCallback((): [number, number][] => {
    if (!curves || curves.length < 48) {
      return [
        [0, 0],
        [0, 0],
        [1, 1],
        [1, 1],
      ];
    }
    return [0, 1, 2, 3].map((p) => {
      const base = channel * 12 + p * 3;
      return [curves[base], curves[base + 1]] as [number, number];
    });
  }, [curves, channel]);

  const weights = useCallback((): number[] => {
    if (!curves || curves.length < 48) return [1, 1, 1, 1];
    return [0, 1, 2, 3].map((p) => curves[channel * 12 + p * 3 + 2]);
  }, [curves, channel]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = SIZE * dpr;
    canvas.height = SIZE * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, SIZE, SIZE);

    const sx = (x: number) => x * SIZE;
    const sy = (y: number) => SIZE - y * SIZE;

    // Grid at quarters.
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let i = 1; i < 4; i++) {
      ctx.moveTo((i * SIZE) / 4, 0);
      ctx.lineTo((i * SIZE) / 4, SIZE);
      ctx.moveTo(0, (i * SIZE) / 4);
      ctx.lineTo(SIZE, (i * SIZE) / 4);
    }
    ctx.stroke();

    // The no-op diagonal, for reference.
    ctx.strokeStyle = "rgba(255,255,255,0.15)";
    ctx.setLineDash([3, 3]);
    ctx.beginPath();
    ctx.moveTo(0, SIZE);
    ctx.lineTo(SIZE, 0);
    ctx.stroke();
    ctx.setLineDash([]);

    // The curve itself, evaluated exactly as the renderer does.
    const p = points();
    const w = weights();
    const evalY = (t: number) => {
      const s = 1 - t;
      const [s2, s3] = [s * s, s * s * s];
      const [t2, t3] = [t * t, t * t * t];
      const nom =
        w[0] * s3 * p[0][1] +
        w[1] * s2 * 3 * t * p[1][1] +
        w[2] * s * 3 * t2 * p[2][1] +
        w[3] * t3 * p[3][1];
      const den = w[0] * s3 + w[1] * s2 * 3 * t + w[2] * s * 3 * t2 + w[3] * t3;
      return den === 0 || !Number.isFinite(den) ? t : nom / den;
    };

    ctx.strokeStyle = CHANNELS[channel].colour;
    ctx.lineWidth = 2;
    ctx.beginPath();
    for (let i = 0; i <= 128; i++) {
      const t = i / 128;
      const y = evalY(t);
      if (i === 0) ctx.moveTo(sx(t), sy(y));
      else ctx.lineTo(sx(t), sy(y));
    }
    ctx.stroke();

    // Control points.
    p.forEach(([x, y], i) => {
      ctx.beginPath();
      ctx.arc(sx(x), sy(y), 5, 0, Math.PI * 2);
      ctx.fillStyle = "#0b0b0b";
      ctx.fill();
      ctx.strokeStyle = CHANNELS[channel].colour;
      ctx.lineWidth = 2;
      ctx.stroke();
      ctx.fillStyle = CHANNELS[channel].colour;
      ctx.font = "9px ui-sans-serif, system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText(String(i + 1), sx(x), sy(y) - 9);
    });
  }, [curves, channel, points, weights]);

  const hit = (mx: number, my: number): number | null => {
    const p = points();
    for (let i = 0; i < 4; i++) {
      const dx = p[i][0] * SIZE - mx;
      const dy = SIZE - p[i][1] * SIZE - my;
      if (Math.hypot(dx, dy) <= 9) return i;
    }
    return null;
  };

  return (
    <div className="space-y-3">
      <div className="flex gap-1">
        {CHANNELS.map((c, i) => (
          <button
            key={c.label}
            onClick={() => setChannel(i)}
            className={`flex-1 rounded px-2 py-1 text-[11px] font-medium transition-colors ${
              channel === i ? "bg-[var(--color-secondary)]" : "hover:bg-[var(--color-accent)]"
            }`}
            style={{ color: channel === i ? c.colour : undefined }}
          >
            {c.label}
          </button>
        ))}
      </div>

      <canvas
        ref={canvasRef}
        style={{ width: SIZE, height: SIZE, touchAction: "none" }}
        className="w-full rounded border border-[var(--color-border)] bg-black/40"
        onPointerDown={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          const scale = SIZE / r.width;
          const i = hit((e.clientX - r.left) * scale, (e.clientY - r.top) * scale);
          if (i === null) return;
          dragging.current = i;
          onInteract(true);
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          const i = dragging.current;
          if (i === null) return;
          const r = e.currentTarget.getBoundingClientRect();
          const scale = SIZE / r.width;
          const x = Math.min(1, Math.max(0, ((e.clientX - r.left) * scale) / SIZE));
          const y = Math.min(1, Math.max(0, 1 - ((e.clientY - r.top) * scale) / SIZE));
          onPoint(channel, i, x, y);
        }}
        onPointerUp={(e) => {
          dragging.current = null;
          onInteract(false);
          if (e.currentTarget.hasPointerCapture(e.pointerId)) {
            e.currentTarget.releasePointerCapture(e.pointerId);
          }
        }}
      />

      <Button size="sm" variant="outline" className="w-full" onClick={() => onReset(channel)}>
        <RotateCcw className="h-3.5 w-3.5" />
        Reset {CHANNELS[channel].label.toLowerCase()}
      </Button>

      <p className="text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
        The overall curve is applied first, then the per-channel one. Only a point's
        <em> height </em> affects the render — the original samples the curve by Bézier
        parameter rather than by x, so dragging sideways changes nothing.
      </p>
    </div>
  );
}
