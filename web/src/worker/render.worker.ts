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
    initPromise = init({ module_or_path: wasmUrl })
      .then(() => {
        post({ type: "ready" });
      })
      .catch((err) => {
        // A failed init (offline, bad cache) must not be memoised forever —
        // clear it so the next message retries instead of erroring until a
        // page reload.
        initPromise = null;
        throw err;
      });
  }
  return initPromise;
}

let palettePromise: Promise<Uint8Array> | null = null;

function ensurePalettes(): Promise<Uint8Array> {
  // Memoised for the same reason as ensureReady: concurrent callers must
  // share one fetch rather than each starting their own.
  if (!palettePromise) {
    // Side-loaded rather than baked into the wasm — it is 538 KB. The URL
    // must come from the Vite base, NOT from self.location: the worker script
    // is emitted under assets/ (and served from src/worker/ in dev), so a
    // script-relative fetch 404s in both environments and the HTML error page
    // would be cached as the "palette blob".
    palettePromise = fetch(`${import.meta.env.BASE_URL}palettes.bin`)
      .then((r) => {
        if (!r.ok) throw new Error(`palettes.bin: HTTP ${r.status}`);
        return r.arrayBuffer();
      })
      .then((b) => {
        palettes = new Uint8Array(b);
        return palettes;
      })
      .catch((err) => {
        // Don't cache the failure forever — allow a retry on the next call.
        palettePromise = null;
        throw err;
      });
  }
  return palettePromise;
}

function apply(h: FlameHandle, p: FlameParams) {
  h.zoom = p.zoom;
  h.scale = p.scale;
  h.angle = p.angle;
  h.setCenter(p.centerX, p.centerY);
  // The UI's chosen output size is the document size a save writes out.
  h.setSize(p.width, p.height);

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
function readInfo(h: FlameHandle): FlameInfo {
  const bg = Array.from(h.background());
  return {
    name: h.name,
    xformCount: h.xformCount,
    hasFinalXform: h.hasFinalXform,
    params: {
      width: h.flameWidth,
      height: h.flameHeight,
      zoom: h.zoom,
      scale: h.scale,
      angle: h.angle,
      centerX: h.centerX,
      centerY: h.centerY,
      brightness: h.brightness,
      gamma: h.gamma,
      vibrancy: h.vibrancy,
      gammaThreshold: h.gammaThreshold,
      // 0-255, straight from the flame — fabricating [0,0,0] here would wipe
      // a loaded file's background on the next render.
      background: [bg[0] ?? 0, bg[1] ?? 0, bg[2] ?? 0],
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
        // Thumbnails from a previous flame must not survive a load — clicking
        // one would replace the new flame with a mutant of the old one.
        mutants = [];
        post({ type: "loaded", id: msg.id, info: readInfo(handle), warnings: [] });
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        post({ type: "curves", id: msg.id, values: Array.from(handle.curves) });
        post({ type: "palette", id: msg.id, rgb: Array.from(handle.paletteBytes()) });
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
        mutants = [];
        post({
          type: "loaded",
          id: msg.id,
          info: readInfo(handle),
          warnings,
        });
        post({ type: "xforms", id: msg.id, xforms: readXforms(handle) });
        post({ type: "curves", id: msg.id, values: Array.from(handle.curves) });
        // The file's own gradient, so the strip shows what actually renders.
        post({ type: "palette", id: msg.id, rgb: Array.from(handle.paletteBytes()) });
        return;
      }

      case "save": {
        if (!handle) {
          // A silent return would leave the caller's `await save()` pending
          // forever — always answer request-style messages.
          post({ type: "error", id: msg.id, message: "save: no flame loaded" });
          return;
        }
        // Sync the UI's params first: while a render is in flight the newest
        // camera/tone values only exist on the main thread, and serialising
        // without them writes a stale file.
        if (msg.params) apply(handle, msg.params);
        post({ type: "saved", id: msg.id, xml: handle.toFlameFile() });
        return;
      }

      case "setPalette": {
        if (!handle) {
          post({ type: "error", id: msg.id, message: "setPalette: no flame loaded" });
          return;
        }
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

      case "setCurvePoint": {
        if (!handle) return;
        handle.setCurvePoint(msg.channel, msg.index, msg.x, msg.y);
        post({ type: "curves", id: msg.id, values: Array.from(handle.curves) });
        return;
      }

      case "resetCurve": {
        if (!handle) return;
        handle.resetCurve(msg.channel);
        post({ type: "curves", id: msg.id, values: Array.from(handle.curves) });
        return;
      }

      case "getCurves": {
        if (!handle) return;
        post({ type: "curves", id: msg.id, values: Array.from(handle.curves) });
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
          // Thumbnail rendering must not leak into document state — slot 4 IS
          // the live flame, and the others may be adopted. Stash and restore
          // the exact values rather than multiplying back (which drifts).
          const q = h.quality;
          const s = h.scale;
          h.quality = msg.quality;
          // Scale pixels-per-unit with the thumbnail size, or a 150px thumb
          // of a flame framed for 512px shows only a small crop of it.
          h.scale = (s * msg.size) / msg.baseSize;
          const px = h.render(msg.size, msg.size);
          h.quality = q;
          h.scale = s;
          thumbs.push(px.buffer as ArrayBuffer);
        }
        post({ type: "mutants", id: msg.id, size: msg.size, thumbs }, thumbs);
        return;
      }

      case "adoptMutant": {
        const chosen = mutants[msg.index];
        if (!chosen) {
          // Stale click (grid already consumed or cleared by a load): say so
          // instead of silently ignoring what looks like a dead button.
          post({ type: "error", id: msg.id, message: "That mutation grid is no longer current — generate a new one." });
          return;
        }
        handle = chosen;
        mutants = [];
        post({ type: "loaded", id: msg.id, info: readInfo(handle), warnings: [] });
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
