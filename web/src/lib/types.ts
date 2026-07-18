/** Everything the renderer needs, sent to the worker on each render. */
export interface FlameParams {
  demo: string;
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

export const DEFAULT_PARAMS: Omit<FlameParams, "demo" | "width" | "height"> = {
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

/** Per-demo camera framing, since each attractor sits in a different region. */
export const DEMOS: Record<string, { label: string; scale: number; centerX: number; centerY: number; quality: number }> = {
  sierpinski: { label: "Sierpinski", scale: 512, centerX: 0.5, centerY: 0.5, quality: 50 },
  spherical: { label: "Spherical Swirl", scale: 180, centerX: 0, centerY: 0, quality: 100 },
};

export type RenderRequest = {
  type: "render";
  id: number;
  params: FlameParams;
};

export type RenderResponse =
  | { type: "ready" }
  | { type: "done"; id: number; width: number; height: number; pixels: ArrayBuffer; ms: number }
  | { type: "error"; id: number; message: string };
