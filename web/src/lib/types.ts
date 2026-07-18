/** Camera, tone and sampling settings — everything the renderer needs. */
export interface FlameParams {
  width: number;
  height: number;

  // Camera
  zoom: number;
  scale: number;
  angle: number;
  centerX: number;
  centerY: number;

  // Tone
  brightness: number;
  gamma: number;
  vibrancy: number;
  gammaThreshold: number;
  background: [number, number, number];

  // Sampling
  quality: number;
  oversample: number;
  filterRadius: number;
}

export const DEFAULT_PARAMS: FlameParams = {
  width: 512,
  height: 512,
  zoom: 0,
  scale: 512,
  angle: 0,
  centerX: 0.5,
  centerY: 0.5,
  brightness: 4,
  gamma: 4,
  vibrancy: 1,
  gammaThreshold: 0,
  background: [0, 0, 0],
  quality: 50,
  oversample: 1,
  filterRadius: 0.5,
};

/** Per-demo framing, since each attractor sits in a different region. */
export const DEMOS: Record<string, { label: string; scale: number; centerX: number; centerY: number; quality: number }> = {
  sierpinski: { label: "Sierpinski", scale: 512, centerX: 0.5, centerY: 0.5, quality: 50 },
  spherical: { label: "Spherical Swirl", scale: 180, centerX: 0, centerY: 0, quality: 100 },
};

/** What the worker currently holds. */
export interface FlameInfo {
  name: string;
  xformCount: number;
  hasFinalXform: boolean;
  params: FlameParams;
}

export type WorkerRequest =
  | { type: "loadDemo"; id: number; name: string }
  | { type: "loadFile"; id: number; xml: string; index: number }
  | { type: "render"; id: number; params: FlameParams }
  | { type: "save"; id: number }
  | { type: "setPalette"; id: number; index: number }
  | { type: "setVariation"; id: number; xform: number; name: string; weight: number };

export type WorkerResponse =
  | { type: "ready" }
  | { type: "loaded"; id: number; info: FlameInfo; warnings: string[] }
  | { type: "done"; id: number; width: number; height: number; pixels: ArrayBuffer; ms: number }
  | { type: "saved"; id: number; xml: string }
  | { type: "palette"; id: number; rgb: number[] }
  | { type: "error"; id: number; message: string };

/**
 * Interaction renders drop quality hard so dragging stays responsive; the
 * full-quality render lands once the drag ends. A full render is ~1s, which
 * is far too slow to run per pointer-move.
 */
export const PREVIEW_QUALITY = 8;
