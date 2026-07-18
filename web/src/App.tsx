import { useCallback, useEffect, useState } from "react";
import { Download, Flame, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ParamSlider } from "@/components/ParamSlider";
import { Viewport } from "@/components/Viewport";
import { useRenderer } from "@/hooks/useRenderer";
import { DEFAULT_PARAMS, DEMOS, type FlameParams } from "@/lib/types";

const PREVIEW_SIZE = 512;

export default function App() {
  const [demo, setDemo] = useState("sierpinski");
  const [params, setParams] = useState<FlameParams>({
    demo: "sierpinski",
    width: PREVIEW_SIZE,
    height: PREVIEW_SIZE,
    ...DEFAULT_PARAMS,
  });

  const { bitmap, ms, rendering, error, render } = useRenderer();

  // Re-render whenever any parameter changes. The worker coalesces bursts, so
  // dragging a slider does not queue up stale frames.
  useEffect(() => {
    render(params);
  }, [params, render]);

  const set = useCallback(<K extends keyof FlameParams>(key: K, value: FlameParams[K]) => {
    setParams((p) => ({ ...p, [key]: value }));
  }, []);

  const selectDemo = useCallback((name: string) => {
    const d = DEMOS[name];
    setDemo(name);
    // Each attractor lives in a different region, so switching must reframe
    // the camera or the flame lands off-screen.
    setParams((p) => ({
      ...p,
      demo: name,
      scale: d.scale,
      centerX: d.centerX,
      centerY: d.centerY,
      quality: d.quality,
      zoom: 0,
      angle: 0,
    }));
  }, []);

  const savePng = useCallback(() => {
    if (!bitmap) return;
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    canvas.getContext("2d")?.putImageData(bitmap, 0, 0);
    canvas.toBlob((blob) => {
      if (!blob) return;
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${demo}.png`;
      a.click();
      URL.revokeObjectURL(url);
    }, "image/png");
  }, [bitmap, demo]);

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      {/* Toolbar */}
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-[var(--color-border)] px-3">
        <div className="flex items-center gap-2">
          <Flame className="h-4 w-4 text-[var(--color-primary)]" />
          <span className="text-sm font-semibold">Apophysis Web</span>
        </div>

        <div className="mx-2 h-5 w-px bg-[var(--color-border)]" />

        <div className="flex items-center gap-1">
          {Object.entries(DEMOS).map(([key, d]) => (
            <Button
              key={key}
              size="sm"
              variant={demo === key ? "default" : "ghost"}
              onClick={() => selectDemo(key)}
            >
              {d.label}
            </Button>
          ))}
        </div>

        <div className="ml-auto flex items-center gap-2">
          <span className="tabular text-xs text-[var(--color-muted-foreground)]">
            {bitmap ? `${bitmap.width}×${bitmap.height} · ${ms.toFixed(0)} ms` : "—"}
          </span>
          <Button size="sm" variant="outline" onClick={() => render(params)} disabled={rendering}>
            <RefreshCw className={rendering ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"} />
            Render
          </Button>
          <Button size="sm" variant="secondary" onClick={savePng} disabled={!bitmap}>
            <Download className="h-3.5 w-3.5" />
            PNG
          </Button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <main className="min-w-0 flex-1">
          <Viewport bitmap={bitmap} rendering={rendering} error={error} />
        </main>

        {/* Adjust panel — the Delphi original's Adjust window, condensed. */}
        <aside className="w-80 shrink-0 overflow-y-auto border-l border-[var(--color-border)] bg-[var(--color-card)] p-3">
          <Tabs defaultValue="camera">
            <TabsList>
              <TabsTrigger value="camera">Camera</TabsTrigger>
              <TabsTrigger value="render">Rendering</TabsTrigger>
              <TabsTrigger value="quality">Quality</TabsTrigger>
            </TabsList>

            <TabsContent value="camera" className="space-y-4">
              <ParamSlider
                label="Zoom"
                value={params.zoom}
                min={-4}
                max={4}
                step={0.05}
                onChange={(v) => set("zoom", v)}
                hint="Powers of two; also scales sample density."
              />
              <ParamSlider
                label="Scale"
                value={params.scale}
                min={10}
                max={2000}
                step={1}
                precision={0}
                onChange={(v) => set("scale", v)}
                hint="Pixels per unit."
              />
              <ParamSlider
                label="Rotation"
                value={params.angle}
                min={-Math.PI}
                max={Math.PI}
                step={0.01}
                precision={3}
                onChange={(v) => set("angle", v)}
                hint="Radians."
              />
              <ParamSlider
                label="Center X"
                value={params.centerX}
                min={-2}
                max={2}
                step={0.005}
                precision={3}
                onChange={(v) => set("centerX", v)}
              />
              <ParamSlider
                label="Center Y"
                value={params.centerY}
                min={-2}
                max={2}
                step={0.005}
                precision={3}
                onChange={(v) => set("centerY", v)}
              />
            </TabsContent>

            <TabsContent value="render" className="space-y-4">
              <ParamSlider
                label="Brightness"
                value={params.brightness}
                min={0}
                max={50}
                step={0.1}
                precision={1}
                onChange={(v) => set("brightness", v)}
              />
              <ParamSlider
                label="Gamma"
                value={params.gamma}
                min={0.1}
                max={10}
                step={0.05}
                onChange={(v) => set("gamma", v)}
              />
              <ParamSlider
                label="Vibrancy"
                value={params.vibrancy}
                min={0}
                max={2}
                step={0.01}
                onChange={(v) => set("vibrancy", v)}
              />
              <ParamSlider
                label="Gamma threshold"
                value={params.gammaThreshold}
                min={0}
                max={0.5}
                step={0.001}
                precision={3}
                onChange={(v) => set("gammaThreshold", v)}
                hint="Linear ramp below this density, to keep sparse pixels from turning to noise."
              />
            </TabsContent>

            <TabsContent value="quality" className="space-y-4">
              <ParamSlider
                label="Quality"
                value={params.quality}
                min={1}
                max={500}
                step={1}
                precision={0}
                onChange={(v) => set("quality", v)}
                hint="Sample density. Higher is cleaner and slower."
              />
              <ParamSlider
                label="Filter radius"
                value={params.filterRadius}
                min={0}
                max={2}
                step={0.05}
                onChange={(v) => set("filterRadius", v)}
              />
              <ParamSlider
                label="Oversample"
                value={params.oversample}
                min={1}
                max={3}
                step={1}
                precision={0}
                onChange={(v) => set("oversample", v)}
                hint="Supersampling factor; cost grows with the square."
              />
              <p className="pt-2 text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
                Rendering runs single-threaded in WebAssembly. GitHub Pages cannot send the
                COOP/COEP headers that multi-threaded WASM needs, so quality above ~200 will
                feel slow.
              </p>
            </TabsContent>
          </Tabs>
        </aside>
      </div>
    </div>
  );
}
