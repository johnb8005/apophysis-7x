import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2, Move, RotateCw, ZoomIn } from "lucide-react";

export type MouseMode = "pan" | "rotate" | "zoom";

interface ViewportProps {
  bitmap: ImageData | null;
  rendering: boolean;
  error: string | null;
  mode: MouseMode;
  onModeChange: (m: MouseMode) => void;
  /** Effective pixels-per-unit, needed to convert drags into world units. */
  ppu: number;
  centerX: number;
  centerY: number;
  angle: number;
  zoom: number;
  onNavigate: (next: Partial<{ centerX: number; centerY: number; angle: number; zoom: number }>) => void;
  onInteract: (active: boolean) => void;
}

/**
 * The main preview, with direct manipulation as in the Delphi original: drag
 * to pan, drag to rotate, scroll or drag to zoom.
 *
 * The canvas is CSS-scaled to fit its pane, so screen pixels are not image
 * pixels — every drag is converted through the displayed/bitmap ratio before
 * being applied in world units.
 */
export function Viewport({
  bitmap,
  rendering,
  error,
  mode,
  onModeChange,
  ppu,
  centerX,
  centerY,
  angle,
  zoom,
  onNavigate,
  onInteract,
}: ViewportProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{
    x: number;
    y: number;
    centerX: number;
    centerY: number;
    angle: number;
    zoom: number;
  } | null>(null);
  const [cursor, setCursor] = useState("grab");

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !bitmap) return;
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    canvas.getContext("2d")?.putImageData(bitmap, 0, 0);
  }, [bitmap]);

  useEffect(() => {
    setCursor(mode === "pan" ? "grab" : mode === "rotate" ? "crosshair" : "zoom-in");
  }, [mode]);

  /** Screen pixels per image pixel, since the canvas is scaled to fit. */
  const displayRatio = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas || !bitmap) return 1;
    const rect = canvas.getBoundingClientRect();
    return rect.width / bitmap.width || 1;
  }, [bitmap]);

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    drag.current = { x: e.clientX, y: e.clientY, centerX, centerY, angle, zoom };
    onInteract(true);
    if (mode === "pan") setCursor("grabbing");
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;

    const ratio = displayRatio();
    const dx = (e.clientX - d.x) / ratio;
    const dy = (e.clientY - d.y) / ratio;

    // Right-drag always orbits, matching the original's 3D camera gesture.
    const effective: MouseMode = e.buttons === 2 ? "rotate" : mode;

    switch (effective) {
      case "pan": {
        // The renderer maps world +y to increasing rows, so both axes move the
        // same way: dragging the image right decreases the camera centre.
        // With a rotated camera the screen axes are the rotated world axes
        // (plot: screen = R(angle)·world), so the drag delta must be rotated
        // back or a horizontal drag pans diagonally.
        const ca = Math.cos(d.angle);
        const sa = Math.sin(d.angle);
        const wx = ca * dx - sa * dy;
        const wy = sa * dx + ca * dy;
        onNavigate({
          centerX: d.centerX - wx / ppu,
          centerY: d.centerY - wy / ppu,
        });
        return;
      }
      case "rotate": {
        // Horizontal travel across the canvas is one full turn.
        const canvas = canvasRef.current;
        const width = canvas ? canvas.width : 512;
        onNavigate({ angle: d.angle + (dx / width) * Math.PI * 2 });
        return;
      }
      case "zoom": {
        // Dragging up zooms in; one canvas height is four powers of two.
        const canvas = canvasRef.current;
        const height = canvas ? canvas.height : 512;
        onNavigate({ zoom: d.zoom - (dy / height) * 4 });
        return;
      }
    }
  };

  const endDrag = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drag.current) return;
    drag.current = null;
    onInteract(false);
    if (mode === "pan") setCursor("grab");
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  // Wheel zoom accumulates locally between renders (the `zoom` prop lags
  // inside a React batch, so `zoom + step` per tick would drop increments),
  // and renders at preview quality until the wheel goes quiet.
  const wheelBase = useRef<{ zoom: number; propZoom: number } | null>(null);
  const wheelIdle = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    if (!wheelBase.current || wheelBase.current.propZoom !== zoom) {
      // (Re)anchor whenever the prop caught up or this is a fresh burst.
      wheelBase.current = { zoom, propZoom: zoom };
    }
    wheelBase.current.zoom += e.deltaY < 0 ? 0.15 : -0.15;
    onInteract(true);
    onNavigate({ zoom: wheelBase.current.zoom });
    if (wheelIdle.current) clearTimeout(wheelIdle.current);
    wheelIdle.current = setTimeout(() => {
      wheelBase.current = null;
      // Ends the interaction, letting the full-quality render land.
      onInteract(false);
    }, 250);
  };

  const modes: [MouseMode, typeof Move, string][] = [
    ["pan", Move, "Pan"],
    ["rotate", RotateCw, "Rotate"],
    ["zoom", ZoomIn, "Zoom"],
  ];

  return (
    <div className="relative flex h-full min-h-0 items-center justify-center bg-black p-4">
      <div
        className="absolute inset-4 rounded opacity-[0.03]"
        style={{
          backgroundImage:
            "linear-gradient(45deg,#fff 25%,transparent 25%,transparent 75%,#fff 75%),linear-gradient(45deg,#fff 25%,transparent 25%,transparent 75%,#fff 75%)",
          backgroundSize: "16px 16px",
          backgroundPosition: "0 0, 8px 8px",
        }}
      />

      {bitmap ? (
        <canvas
          ref={canvasRef}
          className="relative max-h-full max-w-full rounded shadow-2xl ring-1 ring-white/10"
          style={{ cursor, touchAction: "none" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onWheel={onWheel}
          onContextMenu={(e) => e.preventDefault()}
        />
      ) : (
        <div className="relative text-sm text-[var(--color-muted-foreground)]">
          {error ? null : "Initialising renderer…"}
        </div>
      )}

      {/* Mouse-mode toolbar, as in the original's main toolbar. */}
      <div className="absolute left-6 top-6 flex gap-1 rounded-md bg-black/70 p-1 backdrop-blur">
        {modes.map(([m, Icon, label]) => (
          <button
            key={m}
            onClick={() => onModeChange(m)}
            title={`${label} (right-drag always rotates)`}
            className={`rounded p-1.5 transition-colors ${
              mode === m
                ? "bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
                : "text-[var(--color-muted-foreground)] hover:bg-white/10"
            }`}
          >
            <Icon className="h-3.5 w-3.5" />
          </button>
        ))}
      </div>

      {rendering && (
        <div className="absolute right-6 top-6 flex items-center gap-2 rounded-md bg-black/70 px-2.5 py-1.5 text-xs backdrop-blur">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          Rendering
        </div>
      )}

      {error && (
        <div className="absolute inset-x-6 bottom-6 rounded-md border border-[var(--color-destructive)] bg-black/90 px-3 py-2 text-xs text-[var(--color-destructive)]">
          {error}
        </div>
      )}
    </div>
  );
}
