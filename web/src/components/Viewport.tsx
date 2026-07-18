import { useEffect, useRef } from "react";
import { Loader2 } from "lucide-react";

interface ViewportProps {
  bitmap: ImageData | null;
  rendering: boolean;
  error: string | null;
}

/**
 * Draws the rendered frame. The canvas is sized to the bitmap and scaled down
 * by CSS so a large render still fits the pane without resampling in JS.
 */
export function Viewport({ bitmap, rendering, error }: ViewportProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !bitmap) return;
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    canvas.getContext("2d")?.putImageData(bitmap, 0, 0);
  }, [bitmap]);

  return (
    <div className="relative flex h-full min-h-0 items-center justify-center bg-black p-4">
      {/* A checker backdrop makes transparent renders legible. */}
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
          style={{ imageRendering: "auto" }}
        />
      ) : (
        <div className="relative text-sm text-[var(--color-muted-foreground)]">
          {error ? null : "Initialising renderer…"}
        </div>
      )}

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
