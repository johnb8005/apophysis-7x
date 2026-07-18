import { useCallback, useEffect, useRef, useState } from "react";
import type { FlameInfo, FlameParams, WorkerResponse, XformField, XformInfo } from "@/lib/types";

export interface FlameState {
  bitmap: ImageData | null;
  ms: number;
  rendering: boolean;
  error: string | null;
  warnings: string[];
  info: FlameInfo | null;
  palette: number[] | null;
  xforms: XformInfo[];
  variationNames: string[];
}

/**
 * Owns the render worker and the flame it holds.
 *
 * Renders coalesce: while one is in flight only the most recent request is
 * kept, so dragging a slider produces a render at the start and one at the end
 * rather than a backlog of stale frames.
 */
export function useFlame() {
  const workerRef = useRef<Worker | null>(null);
  const nextId = useRef(0);
  const inFlight = useRef<number | null>(null);
  const pending = useRef<FlameParams | null>(null);
  /** Resolvers for request/response pairs (save, load). */
  const waiters = useRef(new Map<number, (msg: WorkerResponse) => void>());

  const [state, setState] = useState<FlameState>({
    bitmap: null,
    ms: 0,
    rendering: false,
    error: null,
    warnings: [],
    info: null,
    palette: null,
    xforms: [],
    variationNames: [],
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
    setState((s) => ({ ...s, rendering: true, error: null }));
    worker.postMessage({ type: "render", id, params });
  }, []);

  /** Send a message and await its matching reply. */
  const request = useCallback((msg: Record<string, unknown>): Promise<WorkerResponse> => {
    const worker = workerRef.current;
    if (!worker) return Promise.reject(new Error("worker not ready"));
    const id = ++nextId.current;
    return new Promise((resolve) => {
      waiters.current.set(id, resolve);
      worker.postMessage({ ...msg, id });
    });
  }, []);

  useEffect(() => {
    const worker = new Worker(new URL("../worker/render.worker.ts", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;

    worker.onmessage = (ev: MessageEvent<WorkerResponse>) => {
      const msg = ev.data;
      if (msg.type === "ready") return;

      // Resolve any awaited request first.
      if ("id" in msg) {
        const waiter = waiters.current.get(msg.id);
        if (waiter) {
          waiters.current.delete(msg.id);
          waiter(msg);
        }
      }

      switch (msg.type) {
        case "error":
          inFlight.current = null;
          setState((s) => ({ ...s, rendering: false, error: msg.message }));
          return;

        case "loaded":
          setState((s) => ({
            ...s,
            info: msg.info,
            warnings: msg.warnings,
            error: null,
          }));
          return;

        case "palette":
          setState((s) => ({ ...s, palette: msg.rgb }));
          return;

        case "xforms":
          setState((s) => ({ ...s, xforms: msg.xforms }));
          return;

        case "variationNames":
          setState((s) => ({ ...s, variationNames: msg.names }));
          return;

        case "done": {
          if (msg.id !== inFlight.current) return;
          inFlight.current = null;
          setState((s) => ({
            ...s,
            bitmap: new ImageData(new Uint8ClampedArray(msg.pixels), msg.width, msg.height),
            ms: msg.ms,
            rendering: false,
            error: null,
          }));
          const queued = pending.current;
          pending.current = null;
          if (queued) render(queued);
          return;
        }
      }
    };

    return () => {
      worker.terminate();
      workerRef.current = null;
      waiters.current.clear();
    };
  }, [render]);

  const loadDemo = useCallback(
    (name: string) => request({ type: "loadDemo", name }),
    [request],
  );

  const loadFile = useCallback(
    (xml: string, index = 0) => request({ type: "loadFile", xml, index }),
    [request],
  );

  const save = useCallback(async (): Promise<string | null> => {
    const msg = await request({ type: "save" });
    return msg.type === "saved" ? msg.xml : null;
  }, [request]);

  const setPalette = useCallback(
    (index: number) => request({ type: "setPalette", index }),
    [request],
  );

  const setVariation = useCallback(
    (xform: number, name: string, weight: number) => {
      workerRef.current?.postMessage({
        type: "setVariation",
        id: ++nextId.current,
        xform,
        name,
        weight,
      });
    },
    [],
  );

  /** Fire-and-forget: the worker echoes an updated xform list where relevant. */
  const send = useCallback((msg: Record<string, unknown>) => {
    workerRef.current?.postMessage({ ...msg, id: ++nextId.current });
  }, []);

  const setCoefs = useCallback(
    (xform: number, coefs: number[]) => send({ type: "setCoefs", xform, coefs }),
    [send],
  );
  const addXform = useCallback(() => send({ type: "addXform" }), [send]);
  const deleteXform = useCallback((xform: number) => send({ type: "deleteXform", xform }), [send]);
  const duplicateXform = useCallback(
    (xform: number) => send({ type: "duplicateXform", xform }),
    [send],
  );
  const refreshXforms = useCallback(() => send({ type: "getXforms" }), [send]);
  const setXformField = useCallback(
    (xform: number, field: XformField, value: number) =>
      send({ type: "setXformField", xform, field, value }),
    [send],
  );
  const loadVariationNames = useCallback(() => send({ type: "variationNames" }), [send]);
  const setXformParam = useCallback(
    (xform: number, variation: string, param: string, value: number) =>
      send({ type: "setXformParam", xform, variation, param, value }),
    [send],
  );
  const setChaos = useCallback(
    (xform: number, to: number, value: number) => send({ type: "setChaos", xform, to, value }),
    [send],
  );

  const dismissWarnings = useCallback(() => {
    setState((s) => ({ ...s, warnings: [] }));
  }, []);

  return {
    ...state,
    render,
    loadDemo,
    loadFile,
    save,
    setPalette,
    setVariation,
    setCoefs,
    addXform,
    deleteXform,
    duplicateXform,
    refreshXforms,
    setXformField,
    loadVariationNames,
    setXformParam,
    setChaos,
    dismissWarnings,
  };
}
