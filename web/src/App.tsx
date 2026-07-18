import { useCallback, useEffect, useRef, useState } from "react";
import { AlertTriangle, Download, Flame, FolderOpen, PanelRight, RefreshCw, Save, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ParamSlider } from "@/components/ParamSlider";
import { PaletteStrip } from "@/components/PaletteStrip";
import { EditorPanel } from "@/components/EditorPanel";
import { TriangleCanvas, type Coefs } from "@/components/TriangleCanvas";
import { Viewport } from "@/components/Viewport";
import { useFlame } from "@/hooks/useFlame";
import { DEFAULT_PARAMS, DEMOS, PREVIEW_QUALITY, type FlameParams } from "@/lib/types";

export default function App() {
  const [params, setParams] = useState<FlameParams>(DEFAULT_PARAMS);
  const [demo, setDemo] = useState("sierpinski");
  const [fileName, setFileName] = useState<string | null>(null);
  const [paletteIndex, setPaletteIndex] = useState(0);

  const flame = useFlame();
  const { render, loadDemo, loadFile, save, setPalette, info, loadVariationNames } = flame;
  const [selectedXform, setSelectedXform] = useState(0);
  const [showEditor, setShowEditor] = useState(true);
  /** Coefs mid-drag, before the worker echoes them back. */
  const [draftCoefs, setDraftCoefs] = useState<Record<number, Coefs>>({});

  /** True while a slider is being dragged — drops quality so it stays live. */
  const [interacting, setInteracting] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  // Load the initial demo and the variation list once the worker is up.
  useEffect(() => {
    loadDemo("sierpinski");
    loadVariationNames();
  }, [loadDemo, loadVariationNames]);

  // Keep the selection in range when transforms are added or removed.
  useEffect(() => {
    if (selectedXform >= flame.xforms.length && flame.xforms.length > 0) {
      setSelectedXform(flame.xforms.length - 1);
    }
  }, [flame.xforms.length, selectedXform]);

  // The worker's echo is authoritative; drop drafts once it arrives.
  useEffect(() => {
    setDraftCoefs({});
  }, [flame.xforms]);

  // Re-render on any parameter change. During interaction, render at low
  // quality; the full-quality frame lands when the drag ends.
  useEffect(() => {
    render(interacting ? { ...params, quality: PREVIEW_QUALITY } : params);
  }, [params, interacting, render]);

  // When a file loads, adopt the flame's own camera and tone settings.
  useEffect(() => {
    if (!info) return;
    setParams((p) => ({ ...info.params, width: p.width, height: p.height }));
  }, [info]);

  const set = useCallback(<K extends keyof FlameParams>(key: K, value: FlameParams[K]) => {
    setParams((p) => ({ ...p, [key]: value }));
  }, []);

  const selectDemo = useCallback(
    async (name: string) => {
      setDemo(name);
      setFileName(null);
      const d = DEMOS[name];
      await loadDemo(name);
      setParams((p) => ({
        ...p,
        scale: d.scale,
        centerX: d.centerX,
        centerY: d.centerY,
        quality: d.quality,
        zoom: 0,
        angle: 0,
      }));
    },
    [loadDemo],
  );

  const openFile = useCallback(
    async (file: File) => {
      const text = await file.text();
      setFileName(file.name);
      await loadFile(text, 0);
    },
    [loadFile],
  );

  const saveFlame = useCallback(async () => {
    const xml = await save();
    if (!xml) return;
    const blob = new Blob([xml], { type: "application/xml" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${info?.name || "flame"}.flame`;
    a.click();
    URL.revokeObjectURL(url);
  }, [save, info]);

  const savePng = useCallback(() => {
    if (!flame.bitmap) return;
    const canvas = document.createElement("canvas");
    canvas.width = flame.bitmap.width;
    canvas.height = flame.bitmap.height;
    canvas.getContext("2d")?.putImageData(flame.bitmap, 0, 0);
    canvas.toBlob((blob) => {
      if (!blob) return;
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${info?.name || demo}.png`;
      a.click();
      URL.revokeObjectURL(url);
    }, "image/png");
  }, [flame.bitmap, info, demo]);

  const pickPalette = useCallback(
    async (index: number) => {
      setPaletteIndex(index);
      await setPalette(index);
      // Force a re-render with the new palette.
      render(params);
    },
    [setPalette, render, params],
  );

  // Triangle coefs: the in-flight draft wins so dragging stays smooth.
  const canvasCoefs: Coefs[] = flame.xforms.map(
    (x, i) => draftCoefs[i] ?? (x.coefs as Coefs),
  );

  const onCoefsChange = useCallback(
    (i: number, next: Coefs, committing: boolean) => {
      setDraftCoefs((d) => ({ ...d, [i]: next }));
      flame.setCoefs(i, next);
      if (committing) {
        setInteracting(false);
        flame.refreshXforms();
        render(params);
      } else {
        setInteracting(true);
        render({ ...params, quality: PREVIEW_QUALITY });
      }
    },
    [flame, params, render],
  );

  const onVariationChange = useCallback(
    (i: number, name: string, weight: number) => {
      flame.setVariation(i, name, weight);
      flame.refreshXforms();
      render(params);
    },
    [flame, params, render],
  );

  const onFieldChange = useCallback(
    (i: number, field: Parameters<typeof flame.setXformField>[1], value: number) => {
      flame.setXformField(i, field, value);
      render(interacting ? { ...params, quality: PREVIEW_QUALITY } : params);
    },
    [flame, params, render, interacting],
  );

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-[var(--color-border)] px-3">
        <div className="flex items-center gap-2">
          <Flame className="h-4 w-4 text-[var(--color-primary)]" />
          <span className="text-sm font-semibold">Apophysis Web</span>
        </div>

        <div className="mx-1 h-5 w-px bg-[var(--color-border)]" />

        <input
          ref={fileInput}
          type="file"
          accept=".flame,.xml,text/xml"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void openFile(f);
            e.target.value = "";
          }}
        />
        <Button size="sm" variant="secondary" onClick={() => fileInput.current?.click()}>
          <FolderOpen className="h-3.5 w-3.5" />
          Open
        </Button>
        <Button size="sm" variant="secondary" onClick={saveFlame}>
          <Save className="h-3.5 w-3.5" />
          Save
        </Button>

        <div className="mx-1 h-5 w-px bg-[var(--color-border)]" />

        <div className="flex items-center gap-1">
          {Object.entries(DEMOS).map(([key, d]) => (
            <Button
              key={key}
              size="sm"
              variant={!fileName && demo === key ? "default" : "ghost"}
              onClick={() => void selectDemo(key)}
            >
              {d.label}
            </Button>
          ))}
        </div>

        <div className="ml-auto flex items-center gap-2">
          <span className="tabular max-w-[22rem] truncate text-xs text-[var(--color-muted-foreground)]">
            {fileName ? `${fileName} — ` : ""}
            {info ? `${info.xformCount} transforms · ` : ""}
            {flame.bitmap ? `${flame.bitmap.width}×${flame.bitmap.height} · ${flame.ms.toFixed(0)} ms` : "—"}
          </span>
          <Button size="sm" variant="outline" onClick={() => render(params)} disabled={flame.rendering}>
            <RefreshCw className={flame.rendering ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"} />
            Render
          </Button>
          <Button size="sm" variant="secondary" onClick={savePng} disabled={!flame.bitmap}>
            <Download className="h-3.5 w-3.5" />
            PNG
          </Button>
          <Button
            size="sm"
            variant={showEditor ? "default" : "ghost"}
            onClick={() => setShowEditor((v) => !v)}
            title="Toggle transform editor"
          >
            <PanelRight className="h-3.5 w-3.5" />
            Editor
          </Button>
        </div>
      </header>

      {flame.warnings.length > 0 && (
        <div className="flex items-start gap-2 border-b border-[var(--color-border)] bg-[var(--color-primary)]/10 px-3 py-2 text-xs">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--color-primary)]" />
          <div className="min-w-0 flex-1">
            <span className="font-medium">Loaded with warnings:</span>{" "}
            {flame.warnings.slice(0, 3).join("; ")}
            {flame.warnings.length > 3 && ` (+${flame.warnings.length - 3} more)`}
          </div>
          <button
            onClick={flame.dismissWarnings}
            className="shrink-0 rounded p-0.5 hover:bg-[var(--color-accent)]"
            aria-label="Dismiss warnings"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <main className="flex min-w-0 flex-1 flex-col">
          <div className="min-h-0 flex-1">
            <Viewport bitmap={flame.bitmap} rendering={flame.rendering} error={flame.error} />
          </div>
          {showEditor && (
            <div className="h-64 shrink-0 border-t border-[var(--color-border)] p-2">
              <TriangleCanvas
                coefs={canvasCoefs}
                selected={selectedXform}
                onSelect={setSelectedXform}
                onChange={onCoefsChange}
              />
            </div>
          )}
        </main>

        <aside className="w-80 shrink-0 overflow-y-auto border-l border-[var(--color-border)] bg-[var(--color-card)] p-3">
          <Tabs defaultValue="editor">
            <TabsList>
              <TabsTrigger value="editor">Editor</TabsTrigger>
              <TabsTrigger value="camera">Camera</TabsTrigger>
              <TabsTrigger value="render">Render</TabsTrigger>
              <TabsTrigger value="gradient">Gradient</TabsTrigger>
              <TabsTrigger value="quality">Quality</TabsTrigger>
            </TabsList>

            <TabsContent value="editor">
              <EditorPanel
                xforms={flame.xforms}
                selected={selectedXform}
                variationNames={flame.variationNames}
                onSelect={setSelectedXform}
                onAdd={() => {
                  flame.addXform();
                  render(params);
                }}
                onDuplicate={(i) => {
                  flame.duplicateXform(i);
                  render(params);
                }}
                onDelete={(i) => {
                  flame.deleteXform(i);
                  render(params);
                }}
                onField={onFieldChange}
                onCoefs={(i, c, committing) => onCoefsChange(i, c as Coefs, committing)}
                onVariation={onVariationChange}
                onInteract={setInteracting}
              />
            </TabsContent>

            <TabsContent value="camera" className="space-y-4">
              <ParamSlider label="Zoom" value={params.zoom} min={-4} max={4} step={0.05}
                onChange={(v) => set("zoom", v)} onInteract={setInteracting}
                hint="Powers of two; also scales sample density." />
              <ParamSlider label="Scale" value={params.scale} min={10} max={2000} step={1} precision={0}
                onChange={(v) => set("scale", v)} onInteract={setInteracting} hint="Pixels per unit." />
              <ParamSlider label="Rotation" value={params.angle} min={-Math.PI} max={Math.PI} step={0.01}
                precision={3} onChange={(v) => set("angle", v)} onInteract={setInteracting} hint="Radians." />
              <ParamSlider label="Center X" value={params.centerX} min={-2} max={2} step={0.005}
                precision={3} onChange={(v) => set("centerX", v)} onInteract={setInteracting} />
              <ParamSlider label="Center Y" value={params.centerY} min={-2} max={2} step={0.005}
                precision={3} onChange={(v) => set("centerY", v)} onInteract={setInteracting} />
            </TabsContent>

            <TabsContent value="render" className="space-y-4">
              <ParamSlider label="Brightness" value={params.brightness} min={0} max={50} step={0.1}
                precision={1} onChange={(v) => set("brightness", v)} onInteract={setInteracting} />
              <ParamSlider label="Gamma" value={params.gamma} min={0.1} max={10} step={0.05}
                onChange={(v) => set("gamma", v)} onInteract={setInteracting} />
              <ParamSlider label="Vibrancy" value={params.vibrancy} min={0} max={2} step={0.01}
                onChange={(v) => set("vibrancy", v)} onInteract={setInteracting} />
              <ParamSlider label="Gamma threshold" value={params.gammaThreshold} min={0} max={0.5}
                step={0.001} precision={3} onChange={(v) => set("gammaThreshold", v)}
                onInteract={setInteracting}
                hint="Linear ramp below this density, to keep sparse pixels from turning to noise." />
            </TabsContent>

            <TabsContent value="gradient">
              <PaletteStrip
                rgb={flame.palette}
                index={paletteIndex}
                onPick={(i) => void pickPalette(i)}
              />
            </TabsContent>

            <TabsContent value="quality" className="space-y-4">
              <ParamSlider label="Quality" value={params.quality} min={1} max={500} step={1} precision={0}
                onChange={(v) => set("quality", v)} onInteract={setInteracting}
                hint="Sample density. Higher is cleaner and slower." />
              <ParamSlider label="Filter radius" value={params.filterRadius} min={0} max={2} step={0.05}
                onChange={(v) => set("filterRadius", v)} onInteract={setInteracting} />
              <ParamSlider label="Oversample" value={params.oversample} min={1} max={3} step={1}
                precision={0} onChange={(v) => set("oversample", v)} onInteract={setInteracting}
                hint="Supersampling factor; cost grows with the square." />
              <p className="pt-2 text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
                Rendering is single-threaded WebAssembly. GitHub Pages cannot send the COOP/COEP
                headers multi-threaded WASM needs, so high quality is slow. Dragging a slider
                renders at reduced quality and refines on release.
              </p>
            </TabsContent>
          </Tabs>
        </aside>
      </div>
    </div>
  );
}
