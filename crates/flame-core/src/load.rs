// `.flame` loading — ported from the live reader in `src/Forms/Main.pas`
// (`XMLScannerStartTag` ~5068, `XMLScannerEmptyTag` ~5330), NOT from
// `src/IO/ParameterIO.pas`, which is compiled but has zero call sites and
// diverges from the real behaviour.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::flame::{Affine, XForm};
use crate::genome::{Flame, Palette};
use crate::registry;
use crate::variation::Variation;
use crate::xml::{self, Element};

/// A non-fatal problem encountered while loading — a missing plugin, an
/// unparseable attribute. The original surfaces these in its Messages window
/// (`LoadTracker.pas`) rather than failing the load.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadWarning {
    pub flame: String,
    pub message: String,
}

/// Result of loading a document: the flames plus anything worth telling the
/// user about.
#[derive(Default)]
pub struct LoadResult {
    pub flames: Vec<Flame>,
    pub warnings: Vec<LoadWarning>,
}

/// The 24 variations whose presence means a flame is already 3D-aware, so its
/// `flatten` slot must be synthesised as 0 rather than 1.
///
/// From `flatten_val` (Main.pas:5237). Note `curl3D_cz` is a *parameter* name,
/// not a variation name — the original tests it as a raw attribute, so we do
/// too.
const FLATTEN_SUPPRESSORS: [&str; 24] = [
    "linear3D",
    "bubble",
    "cylinder",
    "zblur",
    "blur3D",
    "pre_ztranslate",
    "pre_rotate_x",
    "pre_rotate_y",
    "ztranslate",
    "zcone",
    "post_rotate_x",
    "post_rotate_y",
    "julia3D",
    "julia3Dz",
    "curl3D_cz",
    "hemisphere",
    "bwraps2",
    "bwraps",
    "falloff2",
    "crop",
    "pre_falloff2",
    "pre_crop",
    "post_falloff2",
    "post_crop",
];

/// Attributes on `<xform>` that are structural rather than variation weights.
const RESERVED_XFORM_ATTRS: [&str; 14] = [
    "weight",
    "color",
    "symmetry",
    "color_speed",
    "coefs",
    "post",
    "chaos",
    "opacity",
    "plotmode",
    "name",
    "var_color",
    "enabled",
    // Legacy index-based variation notation, handled separately.
    "var",
    "var1",
];

/// Load every flame in a `.flame` document.
pub fn load(input: &str) -> LoadResult {
    let mut result = LoadResult::default();
    for el in xml::find_flames(input) {
        let (flame, warnings) = load_flame(&el);
        result.warnings.extend(warnings);
        result.flames.push(flame);
    }
    result
}

fn load_flame(el: &Element) -> (Flame, Vec<LoadWarning>) {
    let mut f = Flame::default();
    let mut warnings = Vec::new();

    f.name = el.attr("name").unwrap_or("untitled").to_string();

    if let Some(size) = el.attr_floats("size") {
        if size.len() >= 2 {
            f.width = size[0].max(1.0) as usize;
            f.height = size[1].max(1.0) as usize;
        }
    }
    if let Some(c) = el.attr_floats("center") {
        if c.len() >= 2 {
            f.center = [c[0], c[1]];
        }
    }

    if let Some(v) = el.attr_f64("scale") {
        f.pixels_per_unit = v;
    }
    if let Some(v) = el.attr_f64("zoom") {
        f.zoom = v;
    }

    // `rotate` is applied first and `angle` second, so `angle` wins when both
    // are present — which is what Apophysis writes.
    if let Some(v) = el.attr_f64("rotate") {
        f.angle = -core::f64::consts::PI * v / 180.0;
    }
    if let Some(v) = el.attr_f64("angle") {
        f.angle = v;
    }

    // 3D camera.
    if let Some(v) = el.attr_f64("cam_pitch") {
        f.cam_pitch = v;
    }
    if let Some(v) = el.attr_f64("cam_yaw") {
        f.cam_yaw = v;
    }
    // Legacy alias first: cam_dist is the reciprocal of cam_perspective, and
    // the original reads it BEFORE cam_perspective (Main.pas:5133-5136), so
    // cam_perspective wins when a file carries both. (The zero guard is ours;
    // Delphi would fault on 1/0.)
    if let Some(v) = el.attr_f64("cam_dist") {
        if v != 0.0 {
            f.cam_persp = 1.0 / v;
        }
    }
    if let Some(v) = el.attr_f64("cam_perspective") {
        f.cam_persp = v;
    }
    if let Some(v) = el.attr_f64("cam_zpos") {
        f.cam_zpos = v;
    }
    // abs() as in Main.pas:5141 — a negative DOF means the same blur.
    if let Some(v) = el.attr_f64("cam_dof") {
        f.cam_dof = v.abs();
    }

    // Tone. Note `brightness` is stored unscaled — only the legacy *text*
    // parser divides by BRIGHT_ADJUST (ControlPoint.pas:932 vs Main.pas:5109).
    if let Some(v) = el.attr_f64("brightness") {
        f.brightness = v;
    }
    if let Some(v) = el.attr_f64("gamma") {
        f.gamma = v;
    }
    if let Some(v) = el.attr_f64("vibrancy") {
        f.vibrancy = v;
    }
    if let Some(v) = el.attr_f64("gamma_threshold") {
        f.gamma_threshold = v;
    }
    // Background is written normalised 0..1 and read back via Floor(v*255).
    if let Some(bg) = el.attr_floats("background") {
        if bg.len() >= 3 {
            f.background = [
                (bg[0] * 255.0).floor(),
                (bg[1] * 255.0).floor(),
                (bg[2] * 255.0).floor(),
            ];
        }
    }

    // Sampling.
    if let Some(v) = el.attr_f64("quality") {
        f.sample_density = v;
    }
    if let Some(v) = el.attr_usize("oversample") {
        f.spatial_oversample = v.clamp(1, 4);
    }
    if let Some(v) = el.attr_f64("filter") {
        f.spatial_filter_radius = v;
    }
    if let Some(v) = el.attr_i32("soloxform") {
        f.solo_xform = v;
    }

    // `new_linear="1"` means slots 0 and 1 are authoritative. Its ABSENCE
    // means the file predates 7x (or came from flam3) and those slots must be
    // reconstructed — see `synthesise_linear_flatten`.
    let new_linear = el.attr("new_linear") == Some("1");

    // Transforms.
    for xf_el in el.children_named("xform") {
        match load_xform(xf_el, new_linear, &f.name, &mut warnings) {
            Some(xf) => f.xforms.push(xf),
            None => continue,
        }
    }
    for xf_el in el.children_named("finalxform") {
        if let Some(xf) = load_xform(xf_el, new_linear, &f.name, &mut warnings) {
            // `enabled="0"` disables it; absent means enabled.
            f.final_enabled = xf_el.attr("enabled") != Some("0");
            f.final_xform = Some(xf);
        }
    }

    // 4 channels x 4 points x (x, y, weight).
    if let Some(vals) = el.attr_floats("curves") {
        if let Some(c) = crate::curves::parse(&vals) {
            f.curves = c;
        } else if !vals.is_empty() {
            warnings.push(LoadWarning {
                flame: f.name.clone(),
                message: format!("curves attribute had {} values, expected 48", vals.len()),
            });
        }
    }

    if let Some(p) = load_palette(el, &f.name, &mut warnings) {
        f.palette = p;
    }

    (f, warnings)
}

fn load_xform(
    el: &Element,
    new_linear: bool,
    flame_name: &str,
    warnings: &mut Vec<LoadWarning>,
) -> Option<XForm> {
    let mut xf = XForm::default();

    xf.density = el.attr_f64("weight").unwrap_or(0.0);
    xf.color = el.attr_f64("color").unwrap_or(0.0);
    // `symmetry` is the Apophysis name; flam3 writes `color_speed`. The live
    // Apophysis parser does NOT accept color_speed, but we do so flam3 files
    // load — noted as a deliberate divergence.
    xf.symmetry = el
        .attr_f64("symmetry")
        .or_else(|| el.attr_f64("color_speed"))
        .unwrap_or(0.0);
    xf.opacity = el.attr_f64("opacity").unwrap_or(1.0);
    // Legacy: plotmode="off" is an older spelling of opacity 0.
    if el.attr("plotmode") == Some("off") {
        xf.opacity = 0.0;
    }
    xf.plugin_color = el.attr_f64("var_color").unwrap_or(1.0);
    xf.name = el.attr("name").unwrap_or("").to_string();

    if let Some(c) = el.attr_floats("coefs") {
        if c.len() >= 6 {
            xf.coefs = Affine { a: c[0], b: c[1], c: c[2], d: c[3], e: c[4], f: c[5] };
        }
    }
    if let Some(p) = el.attr_floats("post") {
        if p.len() >= 6 {
            xf.post = Affine { a: p[0], b: p[1], c: p[2], d: p[3], e: p[4], f: p[5] };
        }
    }

    // Xaos: a flat, space-separated list truncated at the last non-1.0 entry.
    // Unlisted trailing entries default to 1, and values are absolute-valued.
    if let Some(chaos) = el.attr_floats("chaos") {
        for (i, v) in chaos.iter().enumerate() {
            if i < xf.mod_weights.len() {
                xf.mod_weights[i] = v.abs();
            }
        }
    }

    // Variation weights: every attribute that is not reserved and not a
    // known parameter name is a variation weight.
    let mut vars: Vec<(Box<dyn Variation>, f64)> = Vec::new();
    let mut seen_params: Vec<(String, f64)> = Vec::new();

    for (key, raw) in &el.attrs {
        if RESERVED_XFORM_ATTRS.contains(&key.as_str()) {
            continue;
        }
        let Some(value) = xml::parse_f64(raw) else { continue };

        // Aliases (bwraps2, logn, …) resolve to their canonical name, but the
        // canonical attribute wins when both are present — `ReadWithSubst`
        // tries the canonical name first and only then the alias.
        let canonical = registry::canonical_name(key);
        if canonical != key && el.attr(canonical).is_some() {
            continue;
        }

        if let Some(v) = registry::create(canonical) {
            // linear and flatten are handled below when new_linear is absent.
            if !new_linear && (canonical == "linear" || canonical == "flatten") {
                continue;
            }
            // Two aliases of the same variation (bwraps2 + bwraps7): first
            // one listed wins, matching the subst table's scan order closely
            // enough for a case the original never distinguishes either.
            if !vars.iter().any(|(existing, _)| existing.name() == canonical) {
                vars.push((v, value));
            }
        } else {
            // Not a variation name — stash it as a candidate parameter,
            // under its canonical spelling.
            seen_params.push((canonical.to_string(), value));
        }
    }

    if !new_linear {
        synthesise_linear_flatten(el, &mut vars);
    }

    // Legacy pre-2.0 notation, replacing everything read so far
    // (Main.pas:5464-5482 zeroes all weights first): `var1="N"` sets the
    // variation at registry index N to 1; `var="w0 w1 ..."` lists weights by
    // registry index. Delphi applies the indices against TODAY'S registry,
    // and so do we.
    if let Some(v) = el.attr("var1") {
        if let Ok(idx) = v.trim().parse::<usize>() {
            vars.clear();
            if let Some(name) = registry::all_names().get(idx) {
                if let Some(var) = registry::create(name) {
                    vars.push((var, 1.0));
                }
            }
        }
    }
    if let Some(weights) = el.attr_floats("var") {
        vars.clear();
        let names = registry::all_names();
        for (idx, w) in weights.iter().enumerate() {
            if *w == 0.0 {
                continue;
            }
            if let Some(name) = names.get(idx) {
                if let Some(var) = registry::create(name) {
                    vars.push((var, *w));
                }
            }
        }
    }

    xf.set_variations(vars);

    // Apply parameters now that the variations exist. Anything that matches no
    // attached variation is reported once, since it usually means a missing
    // plugin rather than a typo.
    for (key, value) in seen_params {
        let mut applied = false;
        let names: Vec<&'static str> =
            xf.variations().iter().map(|(v, _)| v.name()).collect();
        for name in names {
            if xf.set_variation_param(name, &key, value).is_some() {
                applied = true;
                break;
            }
        }
        if !applied && looks_like_variation_attr(&key) {
            warnings.push(LoadWarning {
                flame: flame_name.to_string(),
                message: format!("unknown variation or parameter '{key}' — ignored"),
            });
        }
    }

    Some(xf)
}

/// Heuristic: only warn about attributes that plausibly meant something, so a
/// stray `version`/`nick` does not produce noise.
fn looks_like_variation_attr(key: &str) -> bool {
    !matches!(key, "version" | "nick" | "url" | "time" | "notes")
}

/// Reconstruct the `linear` and `flatten` slots for files without
/// `new_linear="1"` — i.e. everything written before Apophysis 7x.15, and
/// everything flam3 produces.
///
/// From `Main.pas:5435`:
///
/// ```text
/// SetVariation(0, linear_val(Attributes));    // linear3D + linear, summed
/// SetVariation(1, flatten_val(Attributes));   // 0 if any 3D var is nonzero
/// ```
///
/// Getting this wrong is silent: old flames render flat or subtly displaced
/// with no error at all.
fn synthesise_linear_flatten(el: &Element, vars: &mut Vec<(Box<dyn Variation>, f64)>) {
    // linear_val: the two spellings are SUMMED, not overridden.
    let linear_val =
        el.attr_f64("linear").unwrap_or(0.0) + el.attr_f64("linear3D").unwrap_or(0.0);

    // flatten_val: 1 unless the flame already does something 3D.
    let is_3d = FLATTEN_SUPPRESSORS.iter().any(|name| {
        el.attr_f64(name).map(|v| v != 0.0).unwrap_or(false)
    });
    let flatten_val = if is_3d { 0.0 } else { 1.0 };

    if linear_val != 0.0 {
        if let Some(v) = registry::create("linear") {
            vars.push((v, linear_val));
        }
    }
    if flatten_val != 0.0 {
        if let Some(v) = registry::create("flatten") {
            vars.push((v, flatten_val));
        }
    }
}

/// Palette, in any of the three formats the original accepts.
fn load_palette(el: &Element, flame_name: &str, warnings: &mut Vec<LoadWarning>) -> Option<Palette> {
    // Preferred: <palette count="256" format="RGB">hex blob</palette>
    if let Some(p) = el.children_named("palette").next() {
        let format = p.attr("format").unwrap_or("RGB");
        let hex: String = p.text.chars().filter(|c| c.is_ascii_hexdigit()).collect();

        let stride = match format {
            "RGB" => 6,
            // RGBA leads with the alpha byte, which is DISCARDED — the
            // original reads from offset i*8 + 2.
            "RGBA" => 8,
            other => {
                warnings.push(LoadWarning {
                    flame: flame_name.to_string(),
                    message: format!("unsupported palette format '{other}'"),
                });
                return None;
            }
        };
        let skip = if stride == 8 { 2 } else { 0 };

        let mut out = Vec::with_capacity(256);
        let bytes = hex.as_bytes();
        let mut i = 0usize;
        while i * stride + skip + 6 <= bytes.len() {
            let at = i * stride + skip;
            let r = u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[at + 2..at + 4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[at + 4..at + 6], 16).unwrap_or(0);
            out.push([r, g, b]);
            i += 1;
        }
        if !out.is_empty() {
            return Some(normalise_palette(out));
        }
    }

    // Legacy: 256 x <color index="N" rgb="R G B"/>
    let colors: Vec<&Element> = el.children_named("color").collect();
    if !colors.is_empty() {
        let mut out = vec![[0u8; 3]; 256];
        for c in colors {
            let idx = c.attr_usize("index").unwrap_or(0).min(255);
            if let Some(rgb) = c.attr_floats("rgb") {
                if rgb.len() >= 3 {
                    out[idx] = [
                        rgb[0].round().clamp(0.0, 255.0) as u8,
                        rgb[1].round().clamp(0.0, 255.0) as u8,
                        rgb[2].round().clamp(0.0, 255.0) as u8,
                    ];
                }
            }
        }
        return Some(Palette(out));
    }

    // Legacy: <colors count="256" data="hex blob"/>
    if let Some(c) = el.children_named("colors").next() {
        if let Some(data) = c.attr("data") {
            let hex: String = data.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
            let mut out = Vec::new();
            let mut i = 0usize;
            while i * 8 + 8 <= hex.len() {
                let at = i * 8 + 2;
                let r = u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[at + 2..at + 4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[at + 4..at + 6], 16).unwrap_or(0);
                out.push([r, g, b]);
                i += 1;
            }
            if !out.is_empty() {
                return Some(normalise_palette(out));
            }
        }
    }

    None
}

/// The renderer indexes with `round(c * 255)`, so the palette must have
/// exactly 256 entries. The original asserts this; we resample instead, so an
/// odd-sized palette loads rather than aborting.
fn normalise_palette(mut entries: Vec<[u8; 3]>) -> Palette {
    if entries.len() == 256 {
        return Palette(entries);
    }
    if entries.is_empty() {
        return Palette::default();
    }
    let src = core::mem::take(&mut entries);
    let n = src.len();
    let out = (0..256)
        .map(|i| src[(i * n / 256).min(n - 1)])
        .collect();
    Palette(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: &str = r#"
<flames name="t">
  <flame name="modern" size="640 480" center="0 0" scale="50" quality="100"
         brightness="4" gamma="4" background="0 0 0" new_linear="1">
    <xform weight="1" color="0" linear="1" coefs="1 0 0 1 0 0" opacity="1"/>
    <xform weight="0.5" color="1" spherical="1" coefs="0.5 0 0 0.5 1 1" opacity="1"/>
  </flame>
</flames>"#;

    #[test]
    fn loads_a_modern_flame() {
        let r = load(MODERN);
        assert_eq!(r.flames.len(), 1);
        let f = &r.flames[0];
        assert_eq!(f.name, "modern");
        assert_eq!((f.width, f.height), (640, 480));
        assert_eq!(f.pixels_per_unit, 50.0);
        assert_eq!(f.sample_density, 100.0);
        assert_eq!(f.xforms.len(), 2);
        assert_eq!(f.xforms[0].density, 1.0);
    }

    /// With new_linear="1", slot values are taken literally and NOT synthesised.
    #[test]
    fn modern_flame_does_not_synthesise_flatten() {
        let f = &load(MODERN).flames[0];
        let names: Vec<&str> = f.xforms[0].variations().iter().map(|(v, _)| v.name()).collect();
        assert!(names.contains(&"linear"));
        assert!(!names.contains(&"flatten"), "flatten must not be synthesised: {names:?}");
    }

    /// The CreateSubstMap aliases: variation names AND parameter names must
    /// resolve. A legacy bwraps2 flame keeps its cellsize; logn maps to log.
    #[test]
    fn alias_substitution_covers_parameters() {
        let doc = r#"<flame name="aliases" size="100 100" new_linear="1">
            <xform weight="1" bwraps2="1" bwraps2_cellsize="0.4" bwraps2_space="0.2"
                   logn="0.5" logn_base="3" Epispiral="1" Epispiral_n="4"
                   coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let r = load(doc);
        assert!(r.warnings.is_empty(), "aliases must not warn: {:?}", r.warnings);
        let xf = &r.flames[0].xforms[0];
        let names: Vec<&str> = xf.variations().iter().map(|(v, _)| v.name()).collect();
        assert!(names.contains(&"bwraps"), "{names:?}");
        assert!(names.contains(&"log"), "{names:?}");
        assert!(names.contains(&"epispiral"), "{names:?}");
        assert_eq!(xf.variation_param("bwraps", "bwraps_cellsize"), Some(0.4));
        assert_eq!(xf.variation_param("bwraps", "bwraps_space"), Some(0.2));
        assert_eq!(xf.variation_param("log", "log_base"), Some(3.0));
        assert_eq!(xf.variation_param("epispiral", "epispiral_n"), Some(4.0));
    }

    /// When a file carries both the canonical attribute and an alias, the
    /// canonical one wins — ReadWithSubst tries it first (Main.pas:7005).
    #[test]
    fn canonical_attribute_beats_alias() {
        let doc = r#"<flame name="both" size="100 100" new_linear="1">
            <xform weight="1" bwraps="1" bwraps_cellsize="0.7" bwraps2_cellsize="0.1"
                   coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let xf = &load(doc).flames[0].xforms[0];
        assert_eq!(xf.variation_param("bwraps", "bwraps_cellsize"), Some(0.7));
    }

    /// bwraps7 is the third spelling of the same plugin.
    #[test]
    fn bwraps7_maps_to_bwraps() {
        let doc = r#"<flame name="b7" size="100 100" new_linear="1">
            <xform weight="1" bwraps7="1" bwraps7_gain="2.5" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let r = load(doc);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let xf = &r.flames[0].xforms[0];
        assert_eq!(xf.variation_param("bwraps", "bwraps_gain"), Some(2.5));
    }

    /// Legacy `var`/`var1` notation lists weights by registry index and
    /// replaces every other variation attribute (Main.pas:5464-5482).
    #[test]
    fn legacy_var_and_var1_notation_load() {
        // var1: single variation by index. Index 2 is sinusoidal.
        let doc = r#"<flame name="v1" size="100 100" new_linear="1">
            <xform weight="1" linear="1" var1="2" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let xf = &load(doc).flames[0].xforms[0];
        let names: Vec<&str> = xf.variations().iter().map(|(v, _)| v.name()).collect();
        assert_eq!(names, vec!["sinusoidal"], "var1 must replace other weights");

        // var: weight list by index (0 = linear, 3 = spherical).
        let doc = r#"<flame name="v" size="100 100" new_linear="1">
            <xform weight="1" swirl="1" var="0.5 0 0 0.25" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let xf = &load(doc).flames[0].xforms[0];
        let mut got: Vec<(&str, f64)> =
            xf.variations().iter().map(|(v, w)| (v.name(), *w)).collect();
        got.sort_by(|a, b| a.0.cmp(b.0));
        assert_eq!(got, vec![("linear", 0.5), ("spherical", 0.25)]);
    }

    /// Without new_linear, a 2D flame must gain flatten=1.
    #[test]
    fn legacy_2d_flame_gains_flatten() {
        let doc = r#"<flame name="old" size="100 100">
            <xform weight="1" linear="1" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        let names: Vec<&str> = f.xforms[0].variations().iter().map(|(v, _)| v.name()).collect();
        assert!(names.contains(&"flatten"), "legacy 2D flame must gain flatten: {names:?}");
        assert!(names.contains(&"linear"));
    }

    /// Without new_linear, a flame using a 3D variation must NOT gain flatten.
    #[test]
    fn legacy_3d_flame_does_not_gain_flatten() {
        let doc = r#"<flame name="old3d" size="100 100">
            <xform weight="1" linear="1" bubble="0.5" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        let names: Vec<&str> = f.xforms[0].variations().iter().map(|(v, _)| v.name()).collect();
        assert!(
            !names.contains(&"flatten"),
            "a 3D flame must not be flattened: {names:?}"
        );
    }

    /// linear and linear3D are SUMMED, not overridden.
    #[test]
    fn legacy_linear_and_linear3d_are_summed() {
        let doc = r#"<flame name="s" size="100 100">
            <xform weight="1" linear="0.4" linear3D="0.6" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        let (_, w) = f.xforms[0]
            .variations()
            .iter()
            .find(|(v, _)| v.name() == "linear")
            .expect("linear missing");
        assert!((w - 1.0).abs() < 1e-12, "expected 0.4+0.6=1.0, got {w}");
        // linear3D is itself a 3D suppressor, so flatten must be absent.
        let names: Vec<&str> = f.xforms[0].variations().iter().map(|(v, _)| v.name()).collect();
        assert!(!names.contains(&"flatten"));
    }

    #[test]
    fn reads_variation_parameters() {
        let doc = r#"<flame name="p" size="100 100" new_linear="1">
            <xform weight="1" julian="1" julian_power="5" julian_dist="0.5" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert_eq!(f.xforms[0].variation_param("julian", "julian_power"), Some(5.0));
        assert_eq!(f.xforms[0].variation_param("julian", "julian_dist"), Some(0.5));
    }

    #[test]
    fn reads_hex_palette() {
        let doc = format!(
            r#"<flame name="pal" size="100 100" new_linear="1">
                 <xform weight="1" linear="1"/>
                 <palette count="256" format="RGB">{}</palette>
               </flame>"#,
            "FF0000".repeat(256)
        );
        let f = &load(&doc).flames[0];
        assert_eq!(f.palette.len(), 256);
        assert_eq!(f.palette.0[0], [255, 0, 0]);
    }

    /// RGBA palettes lead with an alpha byte that must be discarded.
    #[test]
    fn rgba_palette_discards_leading_alpha() {
        let doc = format!(
            r#"<flame name="pal" size="100 100" new_linear="1">
                 <xform weight="1" linear="1"/>
                 <palette count="256" format="RGBA">{}</palette>
               </flame>"#,
            "FF00FF00".repeat(256) // alpha=FF, then 00FF00
        );
        let f = &load(&doc).flames[0];
        assert_eq!(f.palette.0[0], [0, 255, 0], "leading alpha byte was not skipped");
    }

    #[test]
    fn reads_xaos_and_defaults_unlisted_to_one() {
        let doc = r#"<flame name="x" size="100 100" new_linear="1">
            <xform weight="1" linear="1" chaos="0 2"/>
            <xform weight="1" linear="1"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert_eq!(f.xforms[0].mod_weights[0], 0.0);
        assert_eq!(f.xforms[0].mod_weights[1], 2.0);
        assert_eq!(f.xforms[0].mod_weights[2], 1.0, "unlisted entries default to 1");
    }

    /// `rotate` must set the angle, and `angle` wins when both are present.
    #[test]
    fn angle_overrides_rotate() {
        let doc = r#"<flame name="r" size="100 100" angle="0.5" rotate="-90" new_linear="1">
            <xform weight="1" linear="1"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert_eq!(f.angle, 0.5);
        assert_eq!(f.vibrancy, 1.0, "vibrancy must not be clobbered by rotate");

        let doc = r#"<flame name="r" size="100 100" rotate="-180" new_linear="1">
            <xform weight="1" linear="1"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert!((f.angle - core::f64::consts::PI).abs() < 1e-12, "angle: {}", f.angle);
    }

    /// Background is written 0..1 and read via floor(v*255).
    #[test]
    fn background_is_denormalised() {
        let doc = r#"<flame name="b" size="100 100" background="1 0.5 0" new_linear="1">
            <xform weight="1" linear="1"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert_eq!(f.background, [255.0, 127.0, 0.0]);
    }

    /// The malformed `name="x"var_color="y"` output must still load.
    #[test]
    fn loads_malformed_attribute_run() {
        let doc = r#"<flame name="m" size="100 100" new_linear="1">
            <xform weight="1" linear="1" opacity="1" name="foo"var_color="0.25"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert_eq!(f.xforms[0].name, "foo");
        assert_eq!(f.xforms[0].plugin_color, 0.25);
    }

    #[test]
    fn loads_final_xform() {
        let doc = r#"<flame name="f" size="100 100" new_linear="1">
            <xform weight="1" linear="1"/>
            <finalxform color="0" symmetry="1" spherical="1" coefs="1 0 0 1 0 0"/>
        </flame>"#;
        let f = &load(doc).flames[0];
        assert!(f.final_xform.is_some());
        assert!(f.final_enabled);
    }

    #[test]
    fn unknown_variation_produces_a_warning_not_a_failure() {
        let doc = r#"<flame name="u" size="100 100" new_linear="1">
            <xform weight="1" linear="1" some_unknown_plugin="1"/>
        </flame>"#;
        let r = load(doc);
        assert_eq!(r.flames.len(), 1, "load must still succeed");
        assert!(
            r.warnings.iter().any(|w| w.message.contains("some_unknown_plugin")),
            "expected a warning, got {:?}",
            r.warnings
        );
    }
}
