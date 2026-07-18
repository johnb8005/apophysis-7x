import { useMemo, useState } from "react";
import { Copy, Plus, Search, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ParamSlider } from "@/components/ParamSlider";
import type { XformField, XformInfo } from "@/lib/types";

interface EditorPanelProps {
  xforms: XformInfo[];
  selected: number;
  variationNames: string[];
  onSelect: (i: number) => void;
  onAdd: () => void;
  onDuplicate: (i: number) => void;
  onDelete: (i: number) => void;
  onField: (i: number, field: XformField, value: number) => void;
  onCoefs: (i: number, coefs: number[], committing: boolean) => void;
  onVariation: (i: number, name: string, weight: number) => void;
  onInteract: (active: boolean) => void;
}

/**
 * The transform editor's side panel — the Delphi Editor window's tabs, minus
 * the ones that only make sense with a mouse-driven canvas toolbar.
 */
export function EditorPanel({
  xforms,
  selected,
  variationNames,
  onSelect,
  onAdd,
  onDuplicate,
  onDelete,
  onField,
  onCoefs,
  onVariation,
  onInteract,
}: EditorPanelProps) {
  const [search, setSearch] = useState("");
  const [showUnused, setShowUnused] = useState(false);

  const xf = xforms[selected];

  // Variations already attached, so the list can show them first.
  const attached = useMemo(
    () => new Map((xf?.vars ?? []).map((v) => [v.name, v.weight])),
    [xf],
  );

  const visible = useMemo(() => {
    const q = search.trim().toLowerCase();
    return variationNames.filter((n) => {
      if (q && !n.toLowerCase().includes(q)) return false;
      if (!showUnused && !q) return attached.has(n);
      return true;
    });
  }, [variationNames, search, showUnused, attached]);

  if (!xf) {
    return (
      <div className="p-3 text-xs text-[var(--color-muted-foreground)]">
        No transforms loaded.
      </div>
    );
  }

  const coefLabels = ["a", "b", "c", "d", "e", "f"] as const;

  return (
    <div className="space-y-3">
      {/* Transform selector */}
      <div className="space-y-2">
        <div className="flex items-center gap-1">
          <span className="text-xs font-medium">Transforms</span>
          <div className="ml-auto flex gap-1">
            <Button size="icon" variant="ghost" onClick={onAdd} title="Add transform">
              <Plus className="h-3.5 w-3.5" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={() => onDuplicate(selected)}
              title="Duplicate transform"
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={() => onDelete(selected)}
              disabled={xforms.length <= 1}
              title="Delete transform"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap gap-1">
          {xforms.map((_, i) => (
            <button
              key={i}
              onClick={() => onSelect(i)}
              className={`h-7 min-w-7 rounded px-2 text-xs font-medium transition-colors ${
                i === selected
                  ? "text-[var(--color-primary-foreground)]"
                  : "bg-[var(--color-secondary)] hover:bg-[var(--color-accent)]"
              }`}
              style={
                i === selected
                  ? { background: `hsl(${(i * 47) % 360} 85% 60%)` }
                  : { borderLeft: `3px solid hsl(${(i * 47) % 360} 85% 55%)` }
              }
            >
              {i + 1}
            </button>
          ))}
        </div>
      </div>

      <Tabs defaultValue="vars">
        <TabsList>
          <TabsTrigger value="vars">Variations</TabsTrigger>
          <TabsTrigger value="transform">Transform</TabsTrigger>
          <TabsTrigger value="colors">Colors</TabsTrigger>
        </TabsList>

        <TabsContent value="vars" className="space-y-2">
          <div className="relative">
            <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--color-muted-foreground)]" />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={`Search ${variationNames.length} variations…`}
              className="h-8 w-full rounded border border-[var(--color-input)] bg-transparent pl-7 pr-2 text-xs focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
            />
          </div>

          <label className="flex items-center gap-1.5 text-[10px] text-[var(--color-muted-foreground)]">
            <input
              type="checkbox"
              checked={showUnused}
              onChange={(e) => setShowUnused(e.target.checked)}
              className="accent-[var(--color-primary)]"
            />
            Show unused variations
          </label>

          <div className="max-h-64 space-y-1 overflow-y-auto pr-1">
            {visible.length === 0 && (
              <p className="py-2 text-[10px] text-[var(--color-muted-foreground)]">
                No variations attached. Tick “show unused” or search to add one.
              </p>
            )}
            {visible.map((name) => {
              const weight = attached.get(name) ?? 0;
              return (
                <div key={name} className="flex items-center gap-2">
                  <span
                    className={`flex-1 truncate text-[11px] ${
                      weight !== 0 ? "font-medium" : "text-[var(--color-muted-foreground)]"
                    }`}
                    title={name}
                  >
                    {name}
                  </span>
                  <input
                    type="number"
                    step={0.05}
                    value={Number(weight.toFixed(4))}
                    onChange={(e) => {
                      const v = Number.parseFloat(e.target.value);
                      if (Number.isFinite(v)) onVariation(selected, name, v);
                    }}
                    className="tabular h-6 w-16 rounded border border-[var(--color-input)] bg-transparent px-1 text-right text-[11px] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
                  />
                </div>
              );
            })}
          </div>
        </TabsContent>

        <TabsContent value="transform" className="space-y-3">
          <div>
            <p className="mb-1.5 text-[10px] text-[var(--color-muted-foreground)]">
              Affine coefficients. X maps to (a, b), Y to (c, d), origin to (e, f).
            </p>
            <div className="grid grid-cols-2 gap-1.5">
              {coefLabels.map((label, k) => (
                <div key={label} className="flex items-center gap-1.5">
                  <span className="w-3 text-[11px] text-[var(--color-muted-foreground)]">
                    {label}
                  </span>
                  <input
                    type="number"
                    step={0.01}
                    value={Number(xf.coefs[k].toFixed(6))}
                    onChange={(e) => {
                      const v = Number.parseFloat(e.target.value);
                      if (!Number.isFinite(v)) return;
                      const next = [...xf.coefs];
                      next[k] = v;
                      onCoefs(selected, next, true);
                    }}
                    className="tabular h-6 w-full rounded border border-[var(--color-input)] bg-transparent px-1 text-right text-[11px] focus:outline-none focus:ring-1 focus:ring-[var(--color-ring)]"
                  />
                </div>
              ))}
            </div>
            <Button
              size="sm"
              variant="outline"
              className="mt-2 w-full"
              onClick={() => onCoefs(selected, [1, 0, 0, 1, 0, 0], true)}
            >
              Reset to identity
            </Button>
          </div>

          <ParamSlider
            label="Weight"
            value={xf.weight}
            min={0}
            max={10}
            step={0.01}
            onChange={(v) => onField(selected, "weight", v)}
            onInteract={onInteract}
            hint="Selection probability relative to the other transforms."
          />
        </TabsContent>

        <TabsContent value="colors" className="space-y-4">
          <ParamSlider
            label="Color"
            value={xf.color}
            min={0}
            max={1}
            step={0.001}
            precision={3}
            onChange={(v) => onField(selected, "color", v)}
            onInteract={onInteract}
            hint="Position along the gradient."
          />
          <ParamSlider
            label="Color speed"
            value={xf.symmetry}
            min={-1}
            max={1}
            step={0.01}
            onChange={(v) => onField(selected, "symmetry", v)}
            onInteract={onInteract}
            hint="1 freezes the colour, -1 snaps to this transform's colour."
          />
          <ParamSlider
            label="Opacity"
            value={xf.opacity}
            min={0}
            max={1}
            step={0.01}
            onChange={(v) => onField(selected, "opacity", v)}
            onInteract={onInteract}
            hint="Points from this transform are plotted with this probability."
          />
        </TabsContent>
      </Tabs>
    </div>
  );
}
