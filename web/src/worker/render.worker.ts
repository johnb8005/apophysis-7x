/// <reference lib="webworker" />
//
// Rendering runs here rather than on the main thread: a full-quality frame
// takes ~1s, which would freeze every slider drag.

import init, { FlameHandle, flameWarnings } from "@/wasm/flame_core";
import wasmUrl from "@/wasm/flame_core_bg.wasm?url";
import type { FlameInfo, FlameParams, WorkerRequest, WorkerResponse } from "@/lib/types";

let ready = false;
let handle: FlameHandle | null = null;
/** The 701-palette blob, fetched once on demand. */
let palettes: Uint8Array | null = null;

function post(msg: WorkerResponse, transfer?: Transferable[]) {
  (self as DedicatedWorkerGlobalScope).postMessage(msg, transfer ?? []);
}

async function ensureReady() {
  if (ready) return;
  await init({ module_or_path: wasmUrl });
  ready = true;
  post({ type: "ready" });
}

async function ensurePalettes(): Promise<Uint8Array> {
  if (palettes) return palettes;
  // Side-loaded rather than baked into the wasm — it is 538 KB.
  const res = await fetch(new URL("palettes.bin", self.location.href).href);
  palettes = new Uint8Array(await res.arrayBuffer());
  return palettes;
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

/** Read the flame's own settings back out, so loading a file updates the UI. */
function readInfo(h: FlameHandle, width: number, height: number): FlameInfo {
  return {
    name: h.name,
    xformCount: h.xformCount,
    hasFinalXform: h.hasFinalXform,
    params: {
      width,
      height,
      zoom: h.zoom,
      scale: h.scale,
      angle: h.angle,
      centerX: h.centerX,
      centerY: h.centerY,
      brightness: h.brightness,
      gamma: h.gamma,
      vibrancy: h.vibrancy,
      gammaThreshold: h.gammaThreshold,
      background: [0, 0, 0],
      quality: h.quality,
      oversample: h.oversample,
      filterRadius: h.filterRadius,
    },
  };
}

self.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const msg = ev.data;
  try {
    await ensureReady();

    switch (msg.type) {
      case "loadDemo": {
        handle = new FlameHandle(msg.name);
        post({ type: "loaded", id: msg.id, info: readInfo(handle, 512, 512), warnings: [] });
        return;
      }

      case "loadFile": {
        const warnings = Array.from(flameWarnings(msg.xml));
        const h = FlameHandle.fromFlameFile(msg.xml, msg.index);
        if (!h) {
          post({ type: "error", id: msg.id, message: "No flame found in that file." });
          return;
        }
        handle = h;
        post({
          type: "loaded",
          id: msg.id,
          info: readInfo(handle, 512, 512),
          warnings,
        });
        return;
      }

      case "save": {
        if (!handle) return;
        post({ type: "saved", id: msg.id, xml: handle.toFlameFile() });
        return;
      }

      case "setPalette": {
        if (!handle) return;
        const blob = await ensurePalettes();
        handle.setPaletteFromBlob(blob, msg.index);
        post({ type: "palette", id: msg.id, rgb: Array.from(handle.paletteBytes()) });
        return;
      }

      case "setVariation": {
        if (!handle) return;
        handle.setXformVariation(msg.xform, msg.name, msg.weight);
        return;
      }

      case "render": {
        if (!handle) handle = new FlameHandle("sierpinski");
        apply(handle, msg.params);

        const t0 = performance.now();
        const pixels = handle.render(msg.params.width, msg.params.height);
        const ms = performance.now() - t0;

        // Transfer rather than copy — at 1080p this is ~8 MB.
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
        return;
      }
    }
  } catch (e) {
    post({ type: "error", id: msg.id, message: e instanceof Error ? e.message : String(e) });
  }
};
