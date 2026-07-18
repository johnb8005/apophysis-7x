/// <reference lib="webworker" />
//
// Rendering runs here rather than on the main thread: a full-quality frame
// takes ~1s, which would freeze every slider drag.

import init, {
  FlameHandle,
  flameWarnings,
  variationNames,
  variationParams,
} from "@/wasm/flame_core";
import wasmUrl from "@/wasm/flame_core_bg.wasm?url";
import type { FlameInfo, FlameParams, WorkerRequest, WorkerResponse } from "@/lib/types";

/**
 * The in-flight (or completed) module initialisation.
 *
 * This MUST be a memoised promise rather than a boolean flag. `init()` is
 * async, so with a flag every message that arrives before the first init
 * resolves sees `ready === false` and starts its own init. Each extra
 * instantiation gets FRESH linear memory, which silently invalidates every
 * pointer handed out by the earlier instance — the next property set then
 * traps with "memory access out of bounds". The app opens by firing three
 * messages at once, so this raced every time.
 */
let initPromise: Promise<void> | null = null;
let handle: FlameHandle | null = null;
/** Candidates from the last mutation grid, indexed as the 3x3 layout. */
let mutants: FlameHandle[] = [];
/** The 701-palette blob, fetched once on demand. */
let palettes: Uint8Array | null = null;

function post(msg: WorkerResponse, transfer?: Transferable[]) {
  (self as DedicatedWorkerGlobalScope).postMessage(msg, transfer ?? []);
}

function ensureReady(): Promise<void> {
  if (!initPromise) {
    initPromise = init({ module_or_path: wasmUrl }).then(() => {
      post({ type: "ready" });
    });
  }
  return initPromise;
}

let palettePromise: Promise<Uint8Array> | null = null;

function ensurePalettes(): Promise<Uint8Array> {
  // Memoised for the same reason as ensureReady: concurrent callers must
  // share one fetch rather than each starting their own.
  if (!palettePromise) {
    // Side-loaded rather than baked into the wasm — it is 538 KB.
    palettePromise = fetch(new URL("palettes.bin", self.location.href).href)
      .then((r) => r.arrayBuffer())
      .then((b) => {
        palettes = new Uint8Array(b);
        return palettes;
      });
  }
  return palettePromise;
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

/** Per-transform state the editor needs. */
function readXforms(h: FlameHandle) {
  const out = [];
  for (let i = 0; i < h.xformCount; i++) {
    const flat = Array.from(h.xformVariations(i));
    const vars: { name: string; weight: number }[] = [];
    for (let k = 0; k < flat.length; k += 2) {
      vars.push({ name: flat[k], weight: Number(flat[k + 1]) });
    }
    // Per-variation parameters, e.g. julian_power. Only attached variations
    // contribute, so the Variables tab shows exactly what is in play.
    const params: { name: string; value: number; variation: string }[] = [];
    for (const v of vars) {
      if (v.weight === 0) continue;
      for (const pname of Array.from(variationParams(v.name))) {
        const value = h.xformParam(i, v.name, pname);
        if (value !== undefined) {
          params.push({ name: pname, value, variation: v.name });
        }
      }
    }

    out.push({
      coefs: Array.from(h.xformCoefs(i)) as [number, number, number, number, number, number],
      post: Array.from(h.xformPost(i)) as [number, number, number, number, number, number],
      weight: h.xformWeight(i),
      color: h.xformColor(i),
      opacity: h.xformOpacity(i),
      symmetry: h.xformSymmetry(i),
      vars,
      params,
      chaos: Array.from(h.xformChaos(i)),
    });
  }
  return out;
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
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
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
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
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

      case "setCoefs": {
        if (!handle) return;
        const c = msg.coefs;
        handle.setXformCoefs(msg.xform, c[0], c[1], c[2], c[3], c[4], c[5]);
        return;
      }

      case "setPost": {
        if (!handle) return;
        const c = msg.coefs;
        handle.setXformPost(msg.xform, c[0], c[1], c[2], c[3], c[4], c[5]);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "addXform": {
        if (!handle) return;
        handle.addXform();
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "deleteXform": {
        if (!handle) return;
        handle.deleteXform(msg.xform);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "duplicateXform": {
        if (!handle) return;
        handle.duplicateXform(msg.xform);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "getXforms": {
        if (!handle) return;
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "setXformParam": {
        if (!handle) return;
        handle.setXformParam(msg.xform, msg.variation, msg.param, msg.value);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "setChaos": {
        if (!handle) return;
        handle.setXformChaos(msg.xform, msg.to, msg.value);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "setXformField": {
        if (!handle) return;
        if (msg.field === "weight") handle.setXformWeight(msg.xform, msg.value);
        else if (msg.field === "color") handle.setXformColor(msg.xform, msg.value);
        else if (msg.field === "opacity") handle.setXformOpacity(msg.xform, msg.value);
        else if (msg.field === "symmetry") handle.setXformSymmetry(msg.xform, msg.value);
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "mutationGrid": {
        if (!handle) return;
        // Slot 4 is the parent, so the grid reads as "the current flame,
        // surrounded by neighbours" exactly as the original's 3x3 does.
        mutants = [];
        const thumbs: ArrayBuffer[] = [];
        for (let i = 0; i < 9; i++) {
          const h =
            i === 4 ? handle : handle.mutated(msg.trend, msg.amount, msg.seed + i);
          mutants.push(h);
          h.quality = msg.quality;
          // Scale pixels-per-unit with the thumbnail size, or a 150px thumb
          // of a flame framed for 512px shows only a small crop of it.
          h.scale = (h.scale * msg.size) / msg.baseSize;
          const px = h.render(msg.size, msg.size);
          // Restore, so adopting a mutant keeps the parent's framing.
          h.scale = (h.scale * msg.baseSize) / msg.size;
          thumbs.push(px.buffer as ArrayBuffer);
        }
        post({ type: "mutants", id: msg.id, size: msg.size, thumbs }, thumbs);
        return;
      }

      case "adoptMutant": {
        const chosen = mutants[msg.index];
        if (!chosen) return;
        handle = chosen;
        mutants = [];
        post({ type: "loaded", id: msg.id, info: readInfo(handle, 512, 512), warnings: [] });
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        return;
      }

      case "variationNames": {
        post({ type: "variationNames", id: msg.id, names: Array.from(variationNames()) });
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
    const detail = e instanceof Error ? `${e.message}\n${e.stack ?? ""}` : String(e);
    post({ type: "error", id: msg.id, message: `[${msg.type}] ${detail}` });
  }
};
