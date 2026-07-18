/// <reference lib="webworker" />
//
// Rendering runs here rather than on the main thread: a 512x512 frame at
// quality 100 takes well over a second, which would freeze every slider drag.

import init, { FlameHandle } from "@/wasm/flame_core";
import wasmUrl from "@/wasm/flame_core_bg.wasm?url";
import type { FlameParams, RenderRequest, RenderResponse } from "@/lib/types";

let ready = false;
/** One handle per demo, so switching back preserves nothing but costs nothing. */
const handles = new Map<string, FlameHandle>();

function post(msg: RenderResponse, transfer?: Transferable[]) {
  (self as DedicatedWorkerGlobalScope).postMessage(msg, transfer ?? []);
}

function handleFor(demo: string): FlameHandle {
  let h = handles.get(demo);
  if (!h) {
    h = new FlameHandle(demo);
    handles.set(demo, h);
  }
  return h;
}

function apply(h: FlameHandle, p: FlameParams) {
  h.zoom = p.zoom;
  h.scale = p.scale;
  h.angle = p.angle;
  h.setCenter(p.centerX, p.centerY);

  h.brightness = p.brightness;
  h.gamma = p.gamma;
  h.vibrancy = p.vibrancy;
  h.gammaThreshold = p.gammaThreshold;
  h.setBackground(p.background[0], p.background[1], p.background[2]);

  h.quality = p.quality;
  h.oversample = p.oversample;
  h.filterRadius = p.filterRadius;
}

self.onmessage = async (ev: MessageEvent<RenderRequest>) => {
  const msg = ev.data;
  if (msg.type !== "render") return;

  try {
    if (!ready) {
      await init({ module_or_path: wasmUrl });
      ready = true;
      post({ type: "ready" });
    }

    const h = handleFor(msg.params.demo);
    apply(h, msg.params);

    const t0 = performance.now();
    const pixels = h.render(msg.params.width, msg.params.height);
    const ms = performance.now() - t0;

    // Transfer rather than copy — at 1080p this buffer is ~8 MB.
    const buf = pixels.buffer as ArrayBuffer;
    post(
      {
        type: "done",
        id: msg.id,
        width: msg.params.width,
        height: msg.params.height,
        pixels: buf,
        ms,
      },
      [buf],
    );
  } catch (e) {
    post({ type: "error", id: msg.id, message: e instanceof Error ? e.message : String(e) });
  }
};
