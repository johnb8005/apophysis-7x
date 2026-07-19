# Delphi → Rust/Web Port Review

A fidelity and correctness review of the port: `crates/flame-core` (Rust/wasm
renderer) and `web/` (React UI) against the reference Delphi sources in `src/`.
Line references are as of the reviewed revision (`df9da21`).

**Build/test status verified during review:** `cargo test` 85/85 green;
`wasm-pack build` + `tsc -b` + `vite build` all succeed end-to-end.
`cargo clippy` fails out of the box (see Tooling, below).

Severity legend: **Critical** = user-visible wrong behavior in a core flow;
**Major** = wrong output or data loss in realistic scenarios; **Minor** =
edge-case divergence or paper cut; *Note* = documented/acceptable deviation
worth keeping on the radar.

---

## Critical

### C1. Triangle editor is vertically mirrored against the renderer
`web/src/components/TriangleCanvas.tsx:56-77`

The renderer maps world +y to increasing bucket rows, i.e. +y is *down* in
the image (`genome.rs:419` `by = bhs * py`; rows written top-down in
`render.rs`). `toScreen` draws +y *up* (`size.h/2 - wy*scale`). The Delphi
editor applies **two** flips that cancel into y-down: `GetTriangle` negates y
(reference Y vertex `(0,-1)`, both outputs sign-flipped —
`ControlPoint.pas:2799-2806`, `:2683`) and `Editor.pas` `ToScreen` flips again
(`iy - fy*sc`). The web editor has only one flip, so every triangle appears
vertically mirrored relative to the preview, and dragging a handle up moves
the corresponding structure down. The code comment at
`TriangleCanvas.tsx:30-35` claiming this matches the original misreads
`Editor.pas`. (The O=(e,f), X=O+(a,b), Y=O+(c,d) vertex mapping itself is
correct; only the y sign is wrong.)

### C2. Palette switching is broken in production and dev (404)
`web/src/worker/render.worker.ts:53`

`fetch(new URL("palettes.bin", self.location.href))` resolves relative to the
*worker script* URL. In the Pages build the worker is emitted at
`<base>/assets/render.worker-<hash>.js`, so it fetches
`<base>/assets/palettes.bin` — verified 404 on the live site
(`…/apophysis-7x/palettes.bin` is 200, `…/assets/palettes.bin` is 404). In dev
it fetches `/src/worker/palettes.bin`, also 404. There is no `r.ok` check, so
the 404 HTML body is cached as the palette blob: low indices slice HTML bytes
into a garbage palette, higher indices fail silently. Fix: build the URL from
`import.meta.env.BASE_URL` (as `PaletteStrip.tsx:25` already does) and check
`r.ok`.

---

## Major — core (`flame-core`)

### M1. Zoom quality factor applied twice in tone mapping
`crates/flame-core/src/render.rs:125-141` + `:161-162`

Delphi keeps `fcp.actual_density` nominal (`RenderingImplementation.pas:181`)
and multiplies by `sqr(power(2, zoom))` exactly once in
`ImageMaker.CreateImage` (`ImageMaker.pas:448`). The port derives `batches`
from `sample_density * scale²`, back-computes `actual_density` from the batch
count (so it already contains `scale²`), then `tone_map` multiplies by
`scale²` again — net `scale⁴`. Any flame with `zoom ≠ 0` tone-maps too dark
(zoom=1 → k2 4× too small). Correct only at zoom=0.

### M2. Save truncates floats to 6 significant digits; Delphi writes 15
`crates/flame-core/src/save.rs:14`

`g()`'s doc comment says "the way Delphi's %g does: 6 significant digits", but
Delphi's `Format('%g')` defaults to **15** significant digits for Double
(6 digits is C's `%g`, i.e. flam3). Real Apophysis files carry full-precision
coefs; the first Rust re-save truncates every coef/weight/param/curve value,
subtly changing the render. The round-trip tests only feed 6-digit inputs, so
they never see the loss. Curves are worse: Delphi writes them with
`FloatToStr` (15–16 digits, `Main.pas:1861-1879`).

### M3. Alias substitution table mostly missing — legacy flames silently lose parameters
`crates/flame-core/src/registry.rs:35-53`, `load.rs:291-320`

Delphi's `CreateSubstMap`/`ReadWithSubst` (`Main.pas:5429,5443`) maps 32
aliases. The port maps only `cross2`, `bwraps2`/`pre_`/`post_bwraps2`, and
bare `Epispiral`. Missing: `bwraps7*` → `bwraps`, `logn`/`logn_base` → `log`,
and **all parameter aliases** (`bwraps2_cellsize/_space/_gain/_inner_twist/
_outer_twist` incl. pre/post forms, `Epispiral_n/_thickness/_holes`). A legacy
`bwraps2` flame loads the variation but silently keeps default parameters; a
`logn` flame loses the variation entirely. (Found independently by two review
passes.)

### M4. Missing `HasFinalXform` gate: a trivial final xform collapses the image
`crates/flame-core/src/genome.rs:312`, `flame.rs:315-320`

Delphi gates on `FinalXformEnabled and HasFinalXform`
(`ControlPoint.pas:420`, `:2320-2332`), skipping a present-but-trivial final
xform. The port only checks `final_enabled && final_xform.is_some()`. A
`<finalxform>` with identity coefs and all-zero variation weights produces an
empty calc list → every point collapses to the origin, where Delphi renders
normally. `XForm::is_meaningful` is a port of `HasFinalXform` but has **no
callers** (and its `!vars.is_empty()` test is itself wrong vs Delphi, which
tests `linear ≠ 1` / any other weight ≠ 0). Related: `save.rs:156` always
emits `<finalxform>` when present with no `enabled` attribute, and
`load.rs:201` defaults `enabled` to true — so `final_enabled=false` is lost
across save/load (Delphi at least gates emission on `HasFinalXform`).

### M5. Within-pass variation order follows XML attribute order, not registry order
`crates/flame-core/src/flame.rs:206-217`, `load.rs:276-293`

Delphi iterates each pass over the fixed registry index
(`XForm.pas:344-383`; builtins 0–28, then plugins in `Apophysis7X.dpr`
registration order). The port's stable sort by pass preserves *document
attribute order* within a pass. Apophysis-written files happen to list
variations in registry order, so they match by accident; hand-edited or
third-party files (e.g. `pre_spherical` listed before `pre_blur`) compose the
overwriting `pre_`/`post_` transforms in the wrong order and render
differently.

### M6. Legacy flames: synthesized `flatten` runs last in the post pass instead of first
`crates/flame-core/src/load.rs:344-365`

`flatten` is builtin index 1 in Delphi, so it always runs *before* other post
variations. `synthesise_linear_flatten` appends it at the end of `vars`, so it
runs *after* them. A pre-7x.15 flame containing e.g. `post_curl3D` (reads z)
gets z zeroed before it in Delphi but after it in the port — silently
different output.

## Major — web

### M7. Mutation grid permanently clobbers the live flame's quality and size
`web/src/worker/render.worker.ts:288-299` + `crates/flame-core/src/wasm.rs:350-352`

Grid slot 4 *is* the live handle. `h.quality = msg.quality` (14) is never
restored, and `FlameHandle::render` mutates `flame.width/height` to the
thumbnail size (150), also never restored (only `scale` is put back).
Consequences: Save right after generating a grid writes
`quality="14" size="150 150"`; adopting a mutant adopts `quality: 14` into
the UI (`App.tsx:94-97`) and all subsequent renders/saves use it. The
scale save/restore also accumulates FP drift per generation.

### M8. `readInfo` fabricates state, wiping loaded background (and canvas size)
`web/src/worker/render.worker.ts:135`

`background: [0,0,0]` and `width/height: 512` are hardcoded because `wasm.rs`
has `setBackground` but no getter. Loading a flame with `background="1 0.5 0"`
→ UI adopts `[0,0,0]` → next `apply()` calls `setBackground(0,0,0)` → Save
writes `background="0 0 0"`. Similarly the file's own `size` is overwritten by
the preview size on render, so re-saves carry `size="512 512"`. Needs getters
on the wasm handle and a units note (Rust background is 0–255; `lib/types.ts`
doesn't document it).

### M9. Save (and undo snapshots) can serialize stale parameters
`web/src/App.tsx:160-170`, `useFlame.ts:47-59`, `render.worker.ts:63-78`

Camera/tone params reach the worker only via `apply()` at render time. While a
render is in flight the newest params sit in `pending`; a `save` posted then
runs in the worker *before* the pending render, so the XML carries the
previous brightness/gamma/center/zoom. The same staleness poisons
`pushUndo`/`undo` snapshots.

### M10. Undo coverage is partial; redo stack survives non-undoable edits
`web/src/App.tsx:204-236`, `:419-426`, `:496-499`

Triangle/coef drags, weight/color/opacity/symmetry fields, variation
parameters, xaos edits, curve point drags, and palette picks never call
`pushUndo` — Undo after the app's primary gesture (a triangle drag) reverts
an unrelated earlier edit. Because only `pushUndo` clears `redoStack`,
"undo → drag coefs → redo" applies a stale future state on top of the new
edit. Meanwhile `onPost`/`onVariation` push one undo entry per keystroke
(`EditorPanel.tsx:272-277,309-315`), flooding the 50-entry cap.

### M11. Stale mutation grid survives loading a new flame
`web/src/worker/render.worker.ts:29,305-313` + `App.tsx`

Neither `loadFile`/`loadDemo` nor the UI clears `mutants`. After opening a new
file, clicking an old thumbnail replaces the freshly loaded flame with a
mutant of the *previous* one. Conversely, after adopting, the worker clears
`mutants` but the UI still shows the grid; further clicks silently hang their
awaiters (no reply is posted).

### M12. No render cancellation; a wasm panic bricks the worker permanently
`web/src/worker/render.worker.ts:320-342`, `render.rs:118-143`

A render is one synchronous wasm call; while it runs, save/undo/palette/edits
all queue behind it, and changing params does not abandon it. Buckets are
`[f64;4]`: 1080p at oversample 3 is a ~600 MB accumulator; allocation failure
panics, and after a panic the memoised `initPromise` stays resolved with a
poisoned instance — every subsequent message errors until page reload. Needs
batch/yield or terminate-and-respawn, plus a re-init path (a rejected `init`
is also cached forever).

---

## Minor — core

- **`cam_dof` not `abs()`ed on load** (`load.rs:143` vs `Main.pas:5141`);
  negative values mirror the DOF offsets instead of blurring.
- **`cam_dist`/`cam_perspective` precedence inverted** (`load.rs:131-139` vs
  `Main.pas:5133-5137`): when both present, Delphi lets `cam_perspective` win;
  the port lets `cam_dist` win.
- **Legacy `var`/`var1` xform notation unsupported** (`Main.pas:5464-5482`):
  pre-2.0 flames load with *no* variations (points collapse) instead of the
  Delphi behavior.
- **`<symmetry kind="N"/>` element ignored** (`Main.pas:5507-5511`) — dropped
  on load, never re-saved.
- **`palette="N"`/`gradient="N"`/`hue` attributes ignored** (`Main.pas:5094-5107`):
  old index-referenced gradients fall back to the default grey ramp.
- **Non-modelled attributes dropped** (`time`, `nick`, `url`, `estimator_*`,
  `enable_de`, `batches`, `<xdata>`): Rust→Rust round-trip is exact, but
  round-tripping an Apophysis-authored file is lossy. Also a spurious
  "unknown variation" warning for `linear3D` when `new_linear="1"`.
- **Curve "identity" test includes weights** (`curves.rs:27-29` vs
  `ImageMaker.pas:429-437`, which checks only points): default points +
  any weight ≠ 1 is inactive in Delphi but active in the port — and the
  default-point curve is a smoothstep, not a ramp, so it visibly re-tones.
- **Degenerate camera fallback** mutates `cam_w/cam_h` (`render.rs:84-89`)
  where Delphi only substitutes the scale factor — bounds test diverges on
  degenerate input.
- **Gutter clamped at 0** (`render.rs:63` `saturating_sub`) where Delphi
  allows a negative gutter (`RenderingInterface.pas:764`) — one-bucket
  sub-pixel shift for small-radius + high-oversample renders.
- **Filter edge taps skipped instead of clamped** (`render.rs:210-213` vs
  `SafeGetBucket`, `ImageMaker.pas:813-820`) — darker borders when the filter
  exceeds the padding.
- **`xml.rs`**: no numeric character references (`&#NNN;`), no CDATA,
  `<!DOCTYPE …>` with internal subset breaks, non-ASCII text content is
  decoded byte-as-char, recursion unbounded on adversarial nesting. None occur
  in Apophysis-written files.

## Minor — web

- **Hit-testing hits invisible handles of unselected triangles**
  (`TriangleCanvas.tsx:182-202`): a click meant to select can start a drag.
- **Release-commit re-sends the `coefs` prop** (`TriangleCanvas.tsx:281-292`),
  which can lag the final pointermove and snap the triangle back one delta.
- **Gradient strip never shows a loaded flame's palette** — `loadDemo`/`loadFile`
  don't post a `palette` message; `paletteIndex` mislabels embedded palettes.
- **Wheel zoom**: computes from the `zoom` prop (rapid ticks in one React
  batch drop increments), never sets `interacting` so every tick is a
  full-quality render (`Viewport.tsx:130-132`); pan ignores camera rotation
  (`Viewport.tsx:94-101` — rotate `dx/dy` by `-angle`); no DPR handling.
- **No keyboard shortcuts** — Ctrl+Z/Y don't reach undo/redo; no
  Delete/arrow-nudge equivalents from `Editor.pas`.
- **Numeric text inputs**: fire live on partial numbers at full quality,
  ignore `min`/`max`, and `toFixed` rounding can desync display from state
  (`ParamSlider.tsx:42-51`).
- **Final xform invisible in the UI** — round-trips through the model but no
  surface shows, edits, or toggles it (Delphi has `tbEnableFinalXform`).
- **`request`-style worker handlers can hang awaiters** — `save`/`setPalette`
  return without replying when `!handle` (`render.worker.ts:177,183`), leaving
  `await save()` pending forever; waiters cleared on unmount likewise.
- **Any worker error clears coalescing state** (`useFlame.ts:92-94`),
  letting a queued stale render win over a newer frame.
- **Failed file open still sets `fileName`** (`App.tsx:151-158`).
- **CurvesEditor** shows the (inactive) default smoothstep bulging off the
  identity diagonal; editing one channel silently activates the hidden
  smoothstep on the others; curve weights render but can't be edited.
- **`selectDemo` latent race** (`App.tsx:132-149`): the `DEMOS` table's params
  are immediately overwritten by the `loaded` info — benign only while both
  sources agree.

---

## Deviations that are documented and reasonable

- **RNG**: xoshiro256++ instead of the Delphi `RandSeed` LCG, and removal of
  the per-point `Randomize` hack in blur variations — original renders were
  never reproducible; the port gains per-seed determinism. Note the shared
  per-worker Gaussian buffer (vs per-xform in Delphi) correlates the 4-tap
  Irwin-Hall draws across xforms — statistically near-invisible.
- **Density estimation not ported** — dead code upstream
  (`if fcp.enable_de and false`), correctly documented.
- **f64 accumulation buffer**, bounds-checked bucket/palette indexing, zeroed
  `p.z`, Bezier degenerate-denominator fallback — all UB/precision cleanups,
  commented in code.
- **Deterministic parameter defaults** where Delphi randomized in constructors
  (julian power, fan2, rings2, pdj) — only observable when a flame carries a
  weight but omits the parameter attribute.
- **Fixed (rather than preserved) harmless Delphi setter bugs**: auger_scale
  reset, lazysusan_y reset — UI-only in the original, verified harmless.
- **mutate.rs is a rewrite, not a port** ("ported in spirit", per its own
  comment): Delphi interpolates the parent toward fully random flames with a
  trend *variation* (`Mutate.pas:196-345`); the port nudges one gene family
  per mutant from a curated pool. Fine as a design choice, but it is not
  behavior-comparable with the original Mutation window and should be
  described as such.

## Verified faithful (highlights)

- **All 76 variations present** (29 builtins + 47 plugins; 0 missing, 0
  invented), formula-checked including the swapped-argument
  `arctan2(FTx,FTy)`, `FSinA=FTx/len` conventions, EPS guards, julian/
  juliascope integer-power fast paths, bwraps precalc, curl/falloff2/crop
  families, and pinned upstream bugs (blur_zoom setter, flatten-in-post).
- **Chaos loop**: 16 warm-up iterations (`0..=FUSE`), 1024-entry xaos tables
  with identical discretization, opacity rejection placement, solo xform,
  final-xform-doesn't-advance-orbit.
- **Camera and tone mapping** (aside from M1): ppu/zoom, gutter offsets,
  rotation, k1/k2, gamma threshold blend, vibrancy split, background ÷256,
  transparency un-premultiply, filter kernel and normalization.
- **3D projections**: pitch/yaw matrix signs, projection selection precedence,
  DOF coefficient.
- **Curves**: rational-Bezier evaluation, sample-in-t-not-x quirk preserved,
  master-then-channel composition, XML layout.
- **`.flame` I/O**: coefs wire order verified identical; defaults match
  `TXForm.Clear`/reader behavior (brightness unscaled, gamma_threshold 0,
  chaos `Abs` + trailing-1, all three palette formats, RGBA alpha-skip,
  curves layout); correctly ports from the *live* reader in `Main.pas` rather
  than the dead `ParameterIO.pas` (and documents that trap).
- **Web**: memoised wasm-init fix is sound; render coalescing gates `done`
  frames on the in-flight id; affine↔triangle vertex mapping (mod the y sign);
  CurvesEditor evaluation matches `curves.rs` term-for-term; deterministic
  seeds across the boundary (u32, no BigInt issues); no detached-buffer reuse.

## Tooling / process gaps

- **CI never runs `cargo test` or clippy** — `deploy.yml` only builds. The 85
  tests are the port's main safety net; wire them into CI.
- **`cargo clippy` fails out of the box**: 5 deny-by-default
  `clippy::approx_constant` errors on constants copied verbatim from Delphi
  (`misc_a.rs:52,53,56`, `misc_b.rs:56,58`). Add `#[allow]` with a comment.
- **No root workspace `Cargo.toml`** — `cargo test -p flame-core` fails from
  the repo root; either add a workspace or document `cd crates/flame-core`.
- **`bun run build` fails on a fresh clone** (missing generated `@/wasm`);
  chain the `wasm` script or fail with a clear message. `scripts/smoke.mjs`
  exists but isn't wired into CI.
- **README drift**: the "What is ported" list has duplicated entries, and
  "Not yet ported" still lists the mutation grid and curves editor, which
  later commits added.
- **Missing tests that would have caught the above**: save-precision against
  >6-digit inputs; alias substitution (`bwraps2_*`, `logn`, `Epispiral_n`);
  a zoom ≠ 0 brightness-invariance check; curves/finalxform/soloxform/
  `gamma_threshold` round-trips; legacy `<color>`/`<colors>` palettes;
  `cam_*` load/save; `enabled="0"` finalxform.
