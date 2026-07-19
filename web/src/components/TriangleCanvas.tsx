import { useCallback, useEffect, useRef, useState } from "react";

/** One transform's affine, as `[a, b, c, d, e, f]`. */
export type Coefs = [number, number, number, number, number, number];

interface TriangleCanvasProps {
  coefs: Coefs[];
  selected: number;
  onSelect: (i: number) => void;
  /** Called continuously during a drag, then once more on release. */
  onChange: (i: number, coefs: Coefs, committing: boolean) => void;
}

/** Which part of a triangle the pointer grabbed. */
type Grab =
  | { kind: "x" }
  | { kind: "y" }
  | { kind: "o" }
  | { kind: "body"; startWorld: [number, number]; startCoefs: Coefs };

const HANDLE_PX = 7;

/**
 * The transform editor's triangle canvas.
 *
 * Each affine is drawn as a triangle whose vertices are the images of the unit
 * basis: O is the translation `(e, f)`, X is `O + (a, b)`, Y is `O + (c, d)`.
 * Dragging a vertex therefore edits the matrix directly, which is what makes
 * this representation worth having.
 *
 * Stored world +y is drawn DOWNWARD, the same direction the renderer maps it
 * (+y plots to increasing bucket rows, i.e. toward the bottom of the image).
 * The original agrees, via a double negation that is easy to misread as a
 * single flip: `GetTriangle` negates y (ControlPoint.pas:2799-2806, with the
 * reference Y vertex at (0,-1)), and the editor's `ToScreen` flips again
 * (Editor.pas `iy - fy*sc`) — net effect, stored +y is down on screen. With
 * only one flip the editor shows a vertical mirror of the render and dragging
 * a handle up moves the structure down.
 */
export function TriangleCanvas({ coefs, selected, onSelect, onChange }: TriangleCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ scale: 90, ox: 0, oy: 0 });
  const grab = useRef<{ index: number; grab: Grab } | null>(null);
  const lastDraft = useRef<Coefs | null>(null);
  const panning = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);
  const [size, setSize] = useState({ w: 400, h: 400 });

  // Keep the backing store matched to the element's box.
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({ w: Math.max(1, Math.floor(width)), h: Math.max(1, Math.floor(height)) });
    });
    ro.observe(wrap);
    return () => ro.disconnect();
  }, []);

  const toScreen = useCallback(
    (wx: number, wy: number): [number, number] => [
      size.w / 2 + (wx + view.ox) * view.scale,
      size.h / 2 + (wy + view.oy) * view.scale,
    ],
    [size, view],
  );

  const toWorld = useCallback(
    (sx: number, sy: number): [number, number] => [
      (sx - size.w / 2) / view.scale - view.ox,
      (sy - size.h / 2) / view.scale - view.oy,
    ],
    [size, view],
  );

  /** The three triangle vertices for an affine, in world space. */
  const verts = (c: Coefs): { o: [number, number]; x: [number, number]; y: [number, number] } => ({
    o: [c[4], c[5]],
    x: [c[0] + c[4], c[1] + c[5]],
    y: [c[2] + c[4], c[3] + c[5]],
  });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = size.w * dpr;
    canvas.height = size.h * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, size.w, size.h);

    // Grid at unit spacing, fading out when it would be denser than ~12px.
    const step = view.scale >= 12 ? 1 : Math.pow(2, Math.ceil(Math.log2(12 / view.scale)));
    ctx.strokeStyle = "rgba(255,255,255,0.06)";
    ctx.lineWidth = 1;
    const [wx0, wy0] = toWorld(0, 0);
    const [wx1, wy1] = toWorld(size.w, size.h);
    ctx.beginPath();
    for (let x = Math.floor(wx0 / step) * step; x <= wx1; x += step) {
      const [sx] = toScreen(x, 0);
      ctx.moveTo(sx, 0);
      ctx.lineTo(sx, size.h);
    }
    for (let y = Math.floor(wy0 / step) * step; y <= wy1; y += step) {
      const [, sy] = toScreen(0, y);
      ctx.moveTo(0, sy);
      ctx.lineTo(size.w, sy);
    }
    ctx.stroke();

    // Axes.
    ctx.strokeStyle = "rgba(255,255,255,0.22)";
    ctx.beginPath();
    const [ax] = toScreen(0, 0);
    const [, ay] = toScreen(0, 0);
    ctx.moveTo(ax, 0);
    ctx.lineTo(ax, size.h);
    ctx.moveTo(0, ay);
    ctx.lineTo(size.w, ay);
    ctx.stroke();

    // Unit circle, the usual reference for how big a transform is.
    ctx.strokeStyle = "rgba(255,255,255,0.12)";
    ctx.beginPath();
    ctx.arc(ax, ay, view.scale, 0, Math.PI * 2);
    ctx.stroke();

    // Triangles, selected one last so it draws on top.
    const order = coefs.map((_, i) => i).sort((a, b) => (a === selected ? 1 : b === selected ? -1 : 0));
    for (const i of order) {
      const c = coefs[i];
      if (!c) continue;
      const v = verts(c);
      const so = toScreen(...v.o);
      const sx = toScreen(...v.x);
      const sy = toScreen(...v.y);
      const isSel = i === selected;

      const hue = (i * 47) % 360;
      ctx.lineWidth = isSel ? 2 : 1;
      ctx.strokeStyle = `hsl(${hue} 85% ${isSel ? 65 : 45}%)`;
      ctx.fillStyle = `hsl(${hue} 85% 60% / ${isSel ? 0.14 : 0.05})`;

      ctx.beginPath();
      ctx.moveTo(so[0], so[1]);
      ctx.lineTo(sx[0], sx[1]);
      ctx.lineTo(sy[0], sy[1]);
      ctx.closePath();
      ctx.fill();
      ctx.stroke();

      if (isSel) {
        // Handles, labelled as in the original: O origin, X and Y axes.
        const handles: [string, [number, number]][] = [
          ["O", so],
          ["X", sx],
          ["Y", sy],
        ];
        for (const [label, p] of handles) {
          ctx.beginPath();
          ctx.arc(p[0], p[1], HANDLE_PX, 0, Math.PI * 2);
          ctx.fillStyle = "#0b0b0b";
          ctx.fill();
          ctx.strokeStyle = `hsl(${hue} 90% 70%)`;
          ctx.lineWidth = 2;
          ctx.stroke();
          ctx.fillStyle = `hsl(${hue} 90% 78%)`;
          ctx.font = "10px ui-sans-serif, system-ui, sans-serif";
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(label, p[0], p[1]);
        }
      }

      // Index tag near the origin vertex.
      ctx.fillStyle = `hsl(${hue} 85% ${isSel ? 78 : 55}%)`;
      ctx.font = `${isSel ? "bold " : ""}11px ui-sans-serif, system-ui, sans-serif`;
      ctx.textAlign = "left";
      ctx.textBaseline = "bottom";
      ctx.fillText(String(i + 1), so[0] + 10, so[1] - 8);
    }
  }, [coefs, selected, size, view, toScreen, toWorld]);

  const hitTest = useCallback(
    (sx: number, sy: number): { index: number; grab: Grab } | null => {
      // Only the SELECTED triangle's handles are drawn, so only they are
      // grabbable — hit-testing invisible handles of other triangles would
      // turn a click meant to select into an accidental reshape.
      const c0 = coefs[selected];
      if (c0) {
        const v = verts(c0);
        const checks: [Grab, [number, number]][] = [
          [{ kind: "x" }, v.x],
          [{ kind: "y" }, v.y],
          [{ kind: "o" }, v.o],
        ];
        for (const [g, w] of checks) {
          const [hx, hy] = toScreen(...w);
          if (Math.hypot(hx - sx, hy - sy) <= HANDLE_PX + 3) {
            return { index: selected, grab: g };
          }
        }
      }
      // Otherwise, grab the body of whichever triangle contains the point,
      // the selected one first.
      const order = [selected, ...coefs.map((_, i) => i).filter((i) => i !== selected)];
      for (const i of order) {
        const c = coefs[i];
        if (!c) continue;
        const v = verts(c);
        const p = toWorld(sx, sy);
        if (pointInTriangle(p, v.o, v.x, v.y)) {
          return { index: i, grab: { kind: "body", startWorld: p, startCoefs: [...c] as Coefs } };
        }
      }
      return null;
    },
    [coefs, selected, toScreen, toWorld],
  );

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    // Middle button, or empty space, pans the view.
    const hit = e.button === 1 ? null : hitTest(sx, sy);
    if (!hit) {
      panning.current = { x: sx, y: sy, ox: view.ox, oy: view.oy };
      e.currentTarget.setPointerCapture(e.pointerId);
      return;
    }

    if (hit.index !== selected) onSelect(hit.index);
    grab.current = hit;
    lastDraft.current = null;
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    if (panning.current) {
      const dx = (sx - panning.current.x) / view.scale;
      const dy = (sy - panning.current.y) / view.scale;
      setView((v) => ({ ...v, ox: panning.current!.ox + dx, oy: panning.current!.oy + dy }));
      return;
    }

    const g = grab.current;
    if (!g) return;
    const c = coefs[g.index];
    if (!c) return;

    const [wx, wy] = toWorld(sx, sy);
    const next = [...(lastDraft.current ?? c)] as Coefs;

    switch (g.grab.kind) {
      case "o":
        next[4] = wx;
        next[5] = wy;
        break;
      case "x":
        // X vertex is O + (a, b), so dragging it sets the first basis vector.
        next[0] = wx - c[4];
        next[1] = wy - c[5];
        break;
      case "y":
        next[2] = wx - c[4];
        next[3] = wy - c[5];
        break;
      case "body": {
        const dx = wx - g.grab.startWorld[0];
        const dy = wy - g.grab.startWorld[1];
        next[4] = g.grab.startCoefs[4] + dx;
        next[5] = g.grab.startCoefs[5] + dy;
        break;
      }
    }
    lastDraft.current = next;
    onChange(g.index, next, false);
  };

  const endDrag = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (grab.current) {
      // Commit the LAST DRAGGED value, not the coefs prop — the prop can lag
      // the final pointermove by a React render, and committing it would snap
      // the triangle back by one move-delta.
      const c = lastDraft.current ?? coefs[grab.current.index];
      // Commit once on release so the full-quality render happens exactly once.
      if (c) onChange(grab.current.index, [...c] as Coefs, true);
    }
    grab.current = null;
    panning.current = null;
    lastDraft.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const onWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    setView((v) => ({ ...v, scale: Math.min(2000, Math.max(4, v.scale * factor)) }));
  };

  return (
    <div ref={wrapRef} className="relative h-full w-full overflow-hidden rounded bg-black/40">
      <canvas
        ref={canvasRef}
        style={{ width: size.w, height: size.h, touchAction: "none" }}
        className="block cursor-crosshair"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onWheel={onWheel}
      />
      <div className="pointer-events-none absolute bottom-1.5 left-2 text-[10px] text-[var(--color-muted-foreground)]">
        drag handles to reshape · drag body to move · scroll to zoom · drag empty space to pan
      </div>
    </div>
  );
}

/** Barycentric point-in-triangle test. */
function pointInTriangle(
  p: [number, number],
  a: [number, number],
  b: [number, number],
  c: [number, number],
): boolean {
  const d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
  if (Math.abs(d) < 1e-12) return false;
  const u = ((b[1] - c[1]) * (p[0] - c[0]) + (c[0] - b[0]) * (p[1] - c[1])) / d;
  const v = ((c[1] - a[1]) * (p[0] - c[0]) + (a[0] - c[0]) * (p[1] - c[1])) / d;
  return u >= 0 && v >= 0 && u + v <= 1;
}
