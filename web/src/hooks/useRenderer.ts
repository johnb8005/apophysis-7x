import { useCallback, useEffect, useRef, useState } from "react";
import type { FlameParams, RenderResponse } from "@/lib/types";

export interface RenderResult {
  bitmap: ImageData | null;
  ms: number;
  rendering: boolean;
  error: string | null;
}

/**
 * Drives the render worker.
 *
 * Renders are coalesced: while one is in flight, only the most recent request
 * is kept. Dragging a slider therefore produces a render at the start and one
 * at the end rather than a backlog of stale frames.
 */
export function useRenderer() {
  const workerRef = useRef<Worker | null>(null);
  const nextId = useRef(0);
  const inFlight = useRef<number | null>(null);
  const pending = useRef<FlameParams | null>(null);

  const [result, setResult] = useState<RenderResult>({
    bitmap: null,
    ms: 0,
    rendering: false,
    error: null,
  });

  const render = useCallback((params: FlameParams) => {
    const worker = workerRef.current;
    if (!worker) return;

    if (inFlight.current !== null) {
      pending.current = params;
      return;
    }
    const id = ++nextId.current;
    inFlight.current = id;
    setResult((r) => ({ ...r, rendering: true, error: null }));
    worker.postMessage({ type: "render", id, params });
  }, []);

  useEffect(() => {
    const worker = new Worker(new URL("../worker/render.worker.ts", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;

    worker.onmessage = (ev: MessageEvent<RenderResponse>) => {
      const msg = ev.data;
      if (msg.type === "ready") return;

      if (msg.type === "error") {
        inFlight.current = null;
        setResult((r) => ({ ...r, rendering: false, error: msg.message }));
        return;
      }

      // Ignore anything superseded while it was in flight.
      if (msg.id !== inFlight.current) return;
      inFlight.current = null;

      setResult({
        bitmap: new ImageData(new Uint8ClampedArray(msg.pixels), msg.width, msg.height),
        ms: msg.ms,
        rendering: false,
        error: null,
      });

      // Drain the coalesced request, if any.
      const queued = pending.current;
      pending.current = null;
      if (queued) render(queued);
    };

    return () => {
      worker.terminate();
      workerRef.current = null;
    };
  }, [render]);

  return { ...result, render };
}
