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
  /** 0–255 per channel — the unit the Rust renderer stores and expects. */
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

/** One transform, as the editor sees it. */
export interface XformInfo {
  coefs: [number, number, number, number, number, number];
  /** Post transform, applied after the variations. Identity by default. */
  post: [number, number, number, number, number, number];
  weight: number;
  color: number;
  opacity: number;
  symmetry: number;
  vars: { name: string; weight: number }[];
  /** Parameters of the attached variations, e.g. julian_power. */
  params: { name: string; value: number; variation: string }[];
  /** Xaos row: transition weight to each transform. */
  chaos: number[];
}

export type XformField = "weight" | "color" | "opacity" | "symmetry";

export type WorkerRequest =
  | { type: "loadDemo"; id: number; name: string }
  | { type: "loadFile"; id: number; xml: string; index: number }
  | { type: "render"; id: number; params: FlameParams }
  | { type: "save"; id: number; params?: FlameParams }
  | { type: "setPalette"; id: number; index: number }
  | { type: "setVariation"; id: number; xform: number; name: string; weight: number }
  | { type: "setCoefs"; id: number; xform: number; coefs: number[] }
  | { type: "setPost"; id: number; xform: number; coefs: number[] }
  | { type: "addXform"; id: number }
  | { type: "deleteXform"; id: number; xform: number }
  | { type: "duplicateXform"; id: number; xform: number }
  | { type: "getXforms"; id: number }
  | { type: "setXformField"; id: number; xform: number; field: XformField; value: number }
  | { type: "variationNames"; id: number }
  | { type: "setXformParam"; id: number; xform: number; variation: string; param: string; value: number }
  | { type: "setChaos"; id: number; xform: number; to: number; value: number }
  | {
      type: "mutationGrid";
      id: number;
      trend: string;
      amount: number;
      seed: number;
      size: number;
      baseSize: number;
      quality: number;
    }
  | { type: "adoptMutant"; id: number; index: number }
  | { type: "setCurvePoint"; id: number; channel: number; index: number; x: number; y: number }
  | { type: "resetCurve"; id: number; channel: number }
  | { type: "getCurves"; id: number };

export type WorkerResponse =
  | { type: "ready" }
  | { type: "loaded"; id: number; info: FlameInfo; warnings: string[] }
  | { type: "done"; id: number; width: number; height: number; pixels: ArrayBuffer; ms: number }
  | { type: "saved"; id: number; xml: string }
  | { type: "palette"; id: number; rgb: number[] }
  | { type: "xforms"; id: number; xforms: XformInfo[] }
  | { type: "variationNames"; id: number; names: string[] }
  | { type: "mutants"; id: number; size: number; thumbs: ArrayBuffer[] }
  | { type: "curves"; id: number; values: number[] }
  | { type: "error"; id: number; message: string };

/** Mutation trends offered by the UI, mirroring the original's Trend combo. */
export const TRENDS: { value: string; label: string }[] = [
  { value: "random", label: "Random" },
  { value: "coefs", label: "Shape" },
  { value: "weights", label: "Weights" },
  { value: "colors", label: "Colours" },
  { value: "varweights", label: "Variation amounts" },
  { value: "addvar", label: "Add variation" },
];

/**
 * Interaction renders drop quality hard so dragging stays responsive; the
 * full-quality render lands once the drag ends. A full render is ~1s, which
 * is far too slow to run per pointer-move.
 */
export const PREVIEW_QUALITY = 8;

/** Output sizes offered by the render panel. */
export const OUTPUT_SIZES: { label: string; w: number; h: number }[] = [
  { label: "512 × 512", w: 512, h: 512 },
  { label: "800 × 600", w: 800, h: 600 },
  { label: "1024 × 1024", w: 1024, h: 1024 },
  { label: "1280 × 720", w: 1280, h: 720 },
  { label: "1920 × 1080", w: 1920, h: 1080 },
];
