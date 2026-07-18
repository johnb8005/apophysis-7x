// WASM boundary for the browser frontend.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use wasm_bindgen::prelude::*;

use crate::builtins::{Builtin, BuiltinVar};
use crate::flame::{Affine, XForm};
use crate::genome::{Flame, Palette};
use crate::render::render;
use crate::rng::Rng;
use crate::variation::Variation;

/// Panics inside wasm surface as unhelpful "unreachable executed" traps unless
/// a hook forwards them to the console.
#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        web_sys_log(&format!("flame-core panic: {info}"));
    }));
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn web_sys_log(s: &str);
}

fn linear_xform(coefs: Affine, color: f64, density: f64) -> XForm {
    let mut xf = XForm::default();
    xf.coefs = coefs;
    xf.density = density;
    xf.color = color;
    xf.set_variations(vec![(
        Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
        1.0,
    )]);
    xf
}

/// Built-in demo flames, so the UI has something to show before `.flame`
/// loading exists.
fn demo(name: &str) -> Flame {
    let mut flame = Flame::default();
    flame.width = 512;
    flame.height = 512;
    flame.brightness = 4.0;
    flame.gamma = 4.0;
    flame.palette = fire_palette();

    match name {
        "spherical" => {
            let mut a = XForm::default();
            a.coefs = Affine { a: 0.8, b: 0.6, c: -0.6, d: 0.8, e: 0.0, f: 0.0 };
            a.density = 1.0;
            a.color = 0.0;
            a.set_variations(vec![
                (Box::new(BuiltinVar::new(Builtin::Spherical)) as Box<dyn Variation>, 1.0),
                (Box::new(BuiltinVar::new(Builtin::Swirl)) as Box<dyn Variation>, 0.3),
            ]);

            let mut b = XForm::default();
            b.coefs = Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e: 0.6, f: 0.3 };
            b.density = 0.6;
            b.color = 1.0;
            b.set_variations(vec![(
                Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
                1.0,
            )]);

            flame.xforms = vec![a, b];
            flame.center = [0.0, 0.0];
            flame.pixels_per_unit = 180.0;
            flame.sample_density = 100.0;
            flame.name = "Spherical Swirl".into();
        }
        _ => {
            let half = |e: f64, f: f64| Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e, f };
            flame.xforms = vec![
                linear_xform(half(0.0, 0.0), 0.0, 1.0),
                linear_xform(half(0.5, 0.0), 0.5, 1.0),
                linear_xform(half(0.25, 0.5), 1.0, 1.0),
            ];
            flame.center = [0.5, 0.5];
            flame.pixels_per_unit = 512.0;
            flame.sample_density = 50.0;
            flame.name = "Sierpinski".into();
        }
    }
    flame
}

fn fire_palette() -> Palette {
    Palette(
        (0..256)
            .map(|i| {
                let t = i as f64 / 255.0;
                let r = (t * 3.0).min(1.0);
                let g = ((t - 0.33) * 2.2).clamp(0.0, 1.0);
                let b = ((t - 0.72) * 3.5).clamp(0.0, 1.0);
                [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
            })
            .collect(),
    )
}

/// Stateful handle owning one flame. The UI mutates it and re-renders.
#[wasm_bindgen]
pub struct FlameHandle {
    flame: Flame,
    seed: u64,
}

#[wasm_bindgen]
impl FlameHandle {
    /// `name` selects a demo flame: "sierpinski" or "spherical".
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str) -> FlameHandle {
        FlameHandle { flame: demo(name), seed: 0x5EED }
    }

    /// Parse a `.flame` document and take the flame at `index`.
    #[wasm_bindgen(js_name = fromFlameFile)]
    pub fn from_flame_file(text: &str, index: usize) -> Option<FlameHandle> {
        let mut result = crate::load::load(text);
        if index >= result.flames.len() {
            return None;
        }
        Some(FlameHandle { flame: result.flames.swap_remove(index), seed: 0x5EED })
    }

    /// Serialise back to a `.flame` document.
    #[wasm_bindgen(js_name = toFlameFile)]
    pub fn to_flame_file(&self) -> String {
        let title = if self.flame.name.is_empty() { "untitled" } else { &self.flame.name };
        crate::save::write_document(core::slice::from_ref(&self.flame), title)
    }

    /// Replace the palette from a slice of the 701-palette blob.
    ///
    /// `blob` is the raw `palettes.bin` (701 * 256 * 3 bytes); `index` selects
    /// one. Kept out of the wasm binary and side-loaded so the module stays
    /// small.
    #[wasm_bindgen(js_name = setPaletteFromBlob)]
    pub fn set_palette_from_blob(&mut self, blob: &[u8], index: usize) -> bool {
        let start = index * 256 * 3;
        let end = start + 256 * 3;
        if end > blob.len() {
            return false;
        }
        self.flame.palette =
            Palette(blob[start..end].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect());
        true
    }

    #[wasm_bindgen(js_name = hasFinalXform, getter)]
    pub fn has_final_xform(&self) -> bool {
        self.flame.final_xform.is_some() && self.flame.final_enabled
    }

    /// Variations on transform `i` as `[name, weight, name, weight, ...]` —
    /// wasm-bindgen has no clean tuple return and the UI zips them apart.
    #[wasm_bindgen(js_name = xformVariations)]
    pub fn xform_variations(&self, i: usize) -> Vec<String> {
        match self.flame.xforms.get(i) {
            Some(xf) => xf
                .variations()
                .iter()
                .flat_map(|(v, w)| [v.name().to_string(), w.to_string()])
                .collect(),
            None => Vec::new(),
        }
    }

    /// Set a variation's weight on transform `i`, attaching it if absent.
    #[wasm_bindgen(js_name = setXformVariation)]
    pub fn set_xform_variation(&mut self, i: usize, name: &str, weight: f64) -> bool {
        let Some(xf) = self.flame.xforms.get_mut(i) else { return false };
        if xf.set_weight(name, weight) {
            return true;
        }
        match crate::registry::create(name) {
            Some(v) => {
                xf.set_variation(v, weight);
                true
            }
            None => false,
        }
    }

    #[wasm_bindgen(js_name = xformParam)]
    pub fn xform_param(&self, i: usize, var: &str, param: &str) -> Option<f64> {
        self.flame.xforms.get(i).and_then(|xf| xf.variation_param(var, param))
    }

    /// Returns the value actually stored — several variations coerce.
    #[wasm_bindgen(js_name = setXformParam)]
    pub fn set_xform_param(&mut self, i: usize, var: &str, param: &str, v: f64) -> Option<f64> {
        self.flame.xforms.get_mut(i).and_then(|xf| xf.set_variation_param(var, param, v))
    }

    /// Append a transform, seeded as an identity linear map. Returns its index.
    #[wasm_bindgen(js_name = addXform)]
    pub fn add_xform(&mut self) -> usize {
        let mut xf = XForm::default();
        xf.density = 0.5;
        // Spread new transforms around the colour range rather than stacking
        // them all at 0, which is what the original's "add transform" does.
        xf.color = if self.flame.xforms.is_empty() {
            0.0
        } else {
            (self.flame.xforms.len() as f64 * 0.37).fract()
        };
        xf.set_variations(vec![(
            Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
            1.0,
        )]);
        self.flame.xforms.push(xf);
        self.flame.xforms.len() - 1
    }

    /// Duplicate transform `i`, appending the copy.
    #[wasm_bindgen(js_name = duplicateXform)]
    pub fn duplicate_xform(&mut self, i: usize) -> Option<usize> {
        let xf = self.flame.xforms.get(i)?.clone();
        self.flame.xforms.push(xf);
        Some(self.flame.xforms.len() - 1)
    }

    /// Remove transform `i`. The last transform cannot be removed — a flame
    /// with no transforms has nothing to iterate.
    #[wasm_bindgen(js_name = deleteXform)]
    pub fn delete_xform(&mut self, i: usize) -> bool {
        if self.flame.xforms.len() <= 1 || i >= self.flame.xforms.len() {
            return false;
        }
        self.flame.xforms.remove(i);
        true
    }

    /// Post-transform coefficients of transform `i` as `[a, b, c, d, e, f]`.
    #[wasm_bindgen(js_name = xformPost)]
    pub fn xform_post(&self, i: usize) -> Vec<f64> {
        match self.flame.xforms.get(i) {
            Some(xf) => vec![xf.post.a, xf.post.b, xf.post.c, xf.post.d, xf.post.e, xf.post.f],
            None => Vec::new(),
        }
    }

    #[wasm_bindgen(js_name = setXformPost)]
    pub fn set_xform_post(&mut self, i: usize, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.post = Affine { a, b, c, d, e, f };
        }
    }

    #[wasm_bindgen(js_name = xformOpacity)]
    pub fn xform_opacity(&self, i: usize) -> f64 {
        self.flame.xforms.get(i).map(|x| x.opacity).unwrap_or(1.0)
    }

    #[wasm_bindgen(js_name = setXformOpacity)]
    pub fn set_xform_opacity(&mut self, i: usize, v: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.opacity = v.clamp(0.0, 1.0);
        }
    }

    /// Colour speed (XML `symmetry`).
    #[wasm_bindgen(js_name = xformSymmetry)]
    pub fn xform_symmetry(&self, i: usize) -> f64 {
        self.flame.xforms.get(i).map(|x| x.symmetry).unwrap_or(0.0)
    }

    #[wasm_bindgen(js_name = setXformSymmetry)]
    pub fn set_xform_symmetry(&mut self, i: usize, v: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.symmetry = v.clamp(-1.0, 1.0);
        }
    }

    /// The xaos row for transform `i` — transition weights to every transform.
    #[wasm_bindgen(js_name = xformChaos)]
    pub fn xform_chaos(&self, i: usize) -> Vec<f64> {
        let n = self.flame.xforms.len();
        match self.flame.xforms.get(i) {
            Some(xf) => (0..n).map(|j| xf.mod_weights.get(j).copied().unwrap_or(1.0)).collect(),
            None => Vec::new(),
        }
    }

    #[wasm_bindgen(js_name = setXformChaos)]
    pub fn set_xform_chaos(&mut self, i: usize, j: usize, v: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            if j < xf.mod_weights.len() {
                xf.mod_weights[j] = v.max(0.0);
            }
        }
    }

    /// A mutated copy, for the mutation grid.
    ///
    /// `trend` biases the kind of change ("coefs", "weights", "colors",
    /// "varweights", "addvar", or anything else for random). `amount` is the
    /// original's "Speed" control. Seeds are u32 so JS passes a plain number
    /// rather than a BigInt.
    #[wasm_bindgen(js_name = mutated)]
    pub fn mutated(&self, trend: &str, amount: f64, seed: u32) -> FlameHandle {
        FlameHandle {
            flame: crate::mutate::mutate(
                &self.flame,
                crate::mutate::Trend::from_str(trend),
                amount,
                seed as u64,
            ),
            seed: self.seed,
        }
    }

    /// Render at the given size, returning RGBA8 bytes (length w*h*4).
    ///
    /// Width/height override the genome so the UI can render previews cheaply
    /// without mutating document state.
    pub fn render(&mut self, width: usize, height: usize) -> Vec<u8> {
        self.flame.width = width.max(1);
        self.flame.height = height.max(1);

        let mut rng = Rng::new(self.seed);
        self.flame.prepare(&mut rng);
        render(&self.flame, self.seed).data
    }

    // --- Camera ---

    #[wasm_bindgen(getter)]
    pub fn zoom(&self) -> f64 {
        self.flame.zoom
    }
    #[wasm_bindgen(setter)]
    pub fn set_zoom(&mut self, v: f64) {
        self.flame.zoom = v;
    }

    #[wasm_bindgen(getter)]
    pub fn scale(&self) -> f64 {
        self.flame.pixels_per_unit
    }
    #[wasm_bindgen(setter)]
    pub fn set_scale(&mut self, v: f64) {
        self.flame.pixels_per_unit = v.max(0.001);
    }

    #[wasm_bindgen(getter)]
    pub fn angle(&self) -> f64 {
        self.flame.angle
    }
    #[wasm_bindgen(setter)]
    pub fn set_angle(&mut self, v: f64) {
        self.flame.angle = v;
    }

    #[wasm_bindgen(js_name = setCenter)]
    pub fn set_center(&mut self, x: f64, y: f64) {
        self.flame.center = [x, y];
    }

    #[wasm_bindgen(js_name = centerX, getter)]
    pub fn center_x(&self) -> f64 {
        self.flame.center[0]
    }
    #[wasm_bindgen(js_name = centerY, getter)]
    pub fn center_y(&self) -> f64 {
        self.flame.center[1]
    }

    // --- Tone ---

    #[wasm_bindgen(getter)]
    pub fn brightness(&self) -> f64 {
        self.flame.brightness
    }
    #[wasm_bindgen(setter)]
    pub fn set_brightness(&mut self, v: f64) {
        self.flame.brightness = v.max(0.0);
    }

    #[wasm_bindgen(getter)]
    pub fn gamma(&self) -> f64 {
        self.flame.gamma
    }
    #[wasm_bindgen(setter)]
    pub fn set_gamma(&mut self, v: f64) {
        self.flame.gamma = v.max(0.0);
    }

    #[wasm_bindgen(getter)]
    pub fn vibrancy(&self) -> f64 {
        self.flame.vibrancy
    }
    #[wasm_bindgen(setter)]
    pub fn set_vibrancy(&mut self, v: f64) {
        self.flame.vibrancy = v;
    }

    #[wasm_bindgen(js_name = gammaThreshold, getter)]
    pub fn gamma_threshold(&self) -> f64 {
        self.flame.gamma_threshold
    }
    #[wasm_bindgen(js_name = gammaThreshold, setter)]
    pub fn set_gamma_threshold(&mut self, v: f64) {
        self.flame.gamma_threshold = v.max(0.0);
    }

    // --- Sampling ---

    #[wasm_bindgen(getter)]
    pub fn quality(&self) -> f64 {
        self.flame.sample_density
    }
    #[wasm_bindgen(setter)]
    pub fn set_quality(&mut self, v: f64) {
        self.flame.sample_density = v.max(1.0);
    }

    #[wasm_bindgen(getter)]
    pub fn oversample(&self) -> usize {
        self.flame.spatial_oversample
    }
    #[wasm_bindgen(setter)]
    pub fn set_oversample(&mut self, v: usize) {
        self.flame.spatial_oversample = v.clamp(1, 4);
    }

    #[wasm_bindgen(js_name = filterRadius, getter)]
    pub fn filter_radius(&self) -> f64 {
        self.flame.spatial_filter_radius
    }
    #[wasm_bindgen(js_name = filterRadius, setter)]
    pub fn set_filter_radius(&mut self, v: f64) {
        self.flame.spatial_filter_radius = v.max(0.0);
    }

    #[wasm_bindgen(js_name = setBackground)]
    pub fn set_background(&mut self, r: f64, g: f64, b: f64) {
        self.flame.background = [r, g, b];
    }

    // --- Transforms (read-only for now; the editor will extend this) ---

    #[wasm_bindgen(js_name = xformCount, getter)]
    pub fn xform_count(&self) -> usize {
        self.flame.xforms.len()
    }

    /// Affine coefficients of transform `i` as `[a, b, c, d, e, f]`.
    #[wasm_bindgen(js_name = xformCoefs)]
    pub fn xform_coefs(&self, i: usize) -> Vec<f64> {
        match self.flame.xforms.get(i) {
            Some(xf) => {
                vec![xf.coefs.a, xf.coefs.b, xf.coefs.c, xf.coefs.d, xf.coefs.e, xf.coefs.f]
            }
            None => Vec::new(),
        }
    }

    #[wasm_bindgen(js_name = setXformCoefs)]
    pub fn set_xform_coefs(&mut self, i: usize, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.coefs = Affine { a, b, c, d, e, f };
        }
    }

    #[wasm_bindgen(js_name = xformWeight)]
    pub fn xform_weight(&self, i: usize) -> f64 {
        self.flame.xforms.get(i).map(|x| x.density).unwrap_or(0.0)
    }

    #[wasm_bindgen(js_name = setXformWeight)]
    pub fn set_xform_weight(&mut self, i: usize, w: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.density = w.max(0.0);
        }
    }

    #[wasm_bindgen(js_name = xformColor)]
    pub fn xform_color(&self, i: usize) -> f64 {
        self.flame.xforms.get(i).map(|x| x.color).unwrap_or(0.0)
    }

    #[wasm_bindgen(js_name = setXformColor)]
    pub fn set_xform_color(&mut self, i: usize, c: f64) {
        if let Some(xf) = self.flame.xforms.get_mut(i) {
            xf.color = c.clamp(0.0, 1.0);
        }
    }

    /// The 256-entry palette as flat RGB bytes, for drawing the gradient strip.
    #[wasm_bindgen(js_name = paletteBytes)]
    pub fn palette_bytes(&self) -> Vec<u8> {
        self.flame.palette.0.iter().flat_map(|c| c.iter().copied()).collect()
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.flame.name.clone()
    }
}

/// Flame names in a `.flame` document, for populating the file list.
#[wasm_bindgen(js_name = flameNames)]
pub fn flame_names(text: &str) -> Vec<String> {
    crate::load::load(text).flames.iter().map(|f| f.name.clone()).collect()
}

/// Warnings produced while parsing a document — missing plugins and the like.
/// The original shows these in its Messages window rather than failing.
#[wasm_bindgen(js_name = flameWarnings)]
pub fn flame_warnings(text: &str) -> Vec<String> {
    crate::load::load(text)
        .warnings
        .iter()
        .map(|w| format!("{}: {}", w.flame, w.message))
        .collect()
}

/// Every variation name known to this build, for populating UI lists.
#[wasm_bindgen(js_name = variationNames)]
pub fn variation_names() -> Vec<String> {
    crate::registry::all_names().iter().map(|s| s.to_string()).collect()
}

/// The parameter names a given variation declares, e.g. `julian_power`.
#[wasm_bindgen(js_name = variationParams)]
pub fn variation_params(name: &str) -> Vec<String> {
    match crate::registry::create(name) {
        Some(v) => v.param_names().iter().map(|s| s.to_string()).collect(),
        None => Vec::new(),
    }
}
