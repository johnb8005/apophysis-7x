// `.flame` writing — ported from `FlameToXML` (Main.pas:1776) and
// `TXForm.ToXMLString` (XForm.pas:1391).
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::flame::{Affine, XForm};
use crate::genome::Flame;

/// Format a float the way Delphi's `%g` does: 6 significant digits, choosing
/// between fixed and exponential, with trailing zeros stripped.
///
/// Matching this matters for interoperability — Apophysis and flam3 both write
/// `%g`, and tools in the ecosystem expect that shape. It does mean a
/// round-trip loses precision beyond 6 digits, exactly as the original does.
pub fn g(v: f64) -> String {
    if v == 0.0 {
        // Avoid emitting "-0".
        return "0".to_string();
    }
    if !v.is_finite() {
        return "0".to_string();
    }

    const P: i32 = 6;
    let exp = v.abs().log10().floor() as i32;

    let mut s = if exp < -4 || exp >= P {
        // Exponential form with P-1 digits after the point.
        let mut t = format!("{:.*e}", (P - 1) as usize, v);
        // Rust writes `1.5e3`; C writes `1.5e+03`. Normalise toward C so the
        // output matches what other flame tools expect.
        if let Some(pos) = t.find('e') {
            let (mantissa, e) = t.split_at(pos);
            let exp_val: i32 = e[1..].parse().unwrap_or(0);
            let mantissa = strip_trailing_zeros(mantissa);
            t = format!("{mantissa}e{}{:02}", if exp_val < 0 { '-' } else { '+' }, exp_val.abs());
        }
        return t;
    } else {
        format!("{:.*}", (P - 1 - exp).max(0) as usize, v)
    };

    if s.contains('.') {
        s = strip_trailing_zeros(&s);
    }
    s
}

fn strip_trailing_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let t = s.trim_end_matches('0');
    t.trim_end_matches('.').to_string()
}

/// Escape the characters that would break the attribute we embed them in.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn affine_str(a: &Affine) -> String {
    format!("{} {} {} {} {} {}", g(a.a), g(a.b), g(a.c), g(a.d), g(a.e), g(a.f))
}

/// Serialise one flame as a `<flame>` element.
pub fn write_flame(f: &Flame) -> String {
    let mut out = String::new();

    out.push_str(&format!("<flame name=\"{}\" version=\"apophysis-web 0.1\" ", esc(&f.name)));
    out.push_str(&format!("size=\"{} {}\" ", f.width, f.height));
    out.push_str(&format!("center=\"{} {}\" ", g(f.center[0]), g(f.center[1])));
    out.push_str(&format!("scale=\"{}\" ", g(f.pixels_per_unit)));

    if f.angle != 0.0 {
        // Both spellings are written, as the original does: `angle` in radians
        // is authoritative on read, `rotate` in degrees is for flam3 tools.
        out.push_str(&format!("angle=\"{}\" ", g(f.angle)));
        out.push_str(&format!("rotate=\"{}\" ", g(-180.0 * f.angle / core::f64::consts::PI)));
    }
    if f.zoom != 0.0 {
        out.push_str(&format!("zoom=\"{}\" ", g(f.zoom)));
    }

    // 3D camera — omitted when at defaults, matching the original.
    if f.cam_pitch != 0.0 {
        out.push_str(&format!("cam_pitch=\"{}\" ", g(f.cam_pitch)));
    }
    if f.cam_yaw != 0.0 {
        out.push_str(&format!("cam_yaw=\"{}\" ", g(f.cam_yaw)));
    }
    if f.cam_persp != 0.0 {
        out.push_str(&format!("cam_perspective=\"{}\" ", g(f.cam_persp)));
    }
    if f.cam_zpos != 0.0 {
        out.push_str(&format!("cam_zpos=\"{}\" ", g(f.cam_zpos)));
    }
    if f.cam_dof != 0.0 {
        out.push_str(&format!("cam_dof=\"{}\" ", g(f.cam_dof)));
    }

    out.push_str(&format!("oversample=\"{}\" ", f.spatial_oversample));
    out.push_str(&format!("filter=\"{}\" ", g(f.spatial_filter_radius)));
    out.push_str(&format!("quality=\"{}\" ", g(f.sample_density)));

    // Background is written normalised 0..1 and read back via floor(v*255).
    //
    // Naively writing `bg/255` loses a step every round-trip: 127/255 formats
    // to "0.498039" at 6 significant digits, and floor(0.498039*255) is 126.
    // The original has this drift — repeated saves walk the background toward
    // black. Emitting the MIDPOINT of the target bucket instead means floor()
    // recovers the exact value, and the file stays readable by Apophysis (and
    // Apophysis's own files stay readable by us) because the reader is
    // unchanged.
    let bg = |v: f64| g((v + 0.5) / 255.0);
    out.push_str(&format!(
        "background=\"{} {} {}\" ",
        bg(f.background[0]),
        bg(f.background[1]),
        bg(f.background[2])
    ));

    out.push_str(&format!("brightness=\"{}\" ", g(f.brightness)));
    out.push_str(&format!("gamma=\"{}\" ", g(f.gamma)));
    if f.vibrancy != 1.0 {
        out.push_str(&format!("vibrancy=\"{}\" ", g(f.vibrancy)));
    }
    if f.gamma_threshold != 0.0 {
        out.push_str(&format!("gamma_threshold=\"{}\" ", g(f.gamma_threshold)));
    }
    if f.solo_xform >= 0 {
        out.push_str(&format!("soloxform=\"{}\" ", f.solo_xform));
    }

    // Names of the non-builtin variations in use, as the original's `plugins`
    // attribute. Consumers use it to warn about missing plugins.
    let plugins = used_plugins(f);
    out.push_str(&format!("plugins=\"{}\" ", plugins.join(" ")));

    // ESSENTIAL: without this, a reader reconstructs the linear/flatten slots
    // and the flame renders differently. See load.rs.
    out.push_str("new_linear=\"1\"");
    out.push_str(">\n");

    for xf in &f.xforms {
        out.push_str(&write_xform(xf, false));
    }
    if let Some(fx) = &f.final_xform {
        out.push_str(&write_xform(fx, true));
    }

    out.push_str(&write_palette(f));
    out.push_str("</flame>\n");
    out
}

fn used_plugins(f: &Flame) -> Vec<&'static str> {
    let mut names = Vec::new();
    let mut push = |xf: &XForm| {
        for (v, w) in xf.variations() {
            if *w != 0.0 && crate::builtins::Builtin::from_name(v.name()).is_none() {
                let n = v.name();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
    };
    for xf in &f.xforms {
        push(xf);
    }
    if let Some(fx) = &f.final_xform {
        push(fx);
    }
    names
}

fn write_xform(xf: &XForm, final_xform: bool) -> String {
    let tag = if final_xform { "finalxform" } else { "xform" };
    let mut out = format!("   <{tag} ");

    if !final_xform {
        out.push_str(&format!("weight=\"{}\" ", g(xf.density)));
    }
    out.push_str(&format!("color=\"{}\" ", g(xf.color)));
    if xf.symmetry != 0.0 {
        out.push_str(&format!("symmetry=\"{}\" ", g(xf.symmetry)));
    }

    // Variation weights, then their parameters — matching the original's
    // ordering so diffs against Apophysis output stay readable.
    for (v, w) in xf.variations() {
        if *w != 0.0 {
            out.push_str(&format!("{}=\"{}\" ", v.name(), g(*w)));
        }
    }

    out.push_str(&format!("coefs=\"{}\" ", affine_str(&xf.coefs)));
    if !xf.post.is_identity() {
        out.push_str(&format!("post=\"{}\" ", affine_str(&xf.post)));
    }

    for (v, w) in xf.variations() {
        if *w == 0.0 {
            continue;
        }
        for p in v.param_names() {
            if let Some(val) = v.get_param(p) {
                out.push_str(&format!("{p}=\"{}\" ", g(val)));
            }
        }
    }

    // Xaos, truncated at the last non-1.0 entry and omitted when all are 1.
    if let Some(last) = xf.mod_weights.iter().rposition(|w| *w != 1.0) {
        out.push_str("chaos=\"");
        for w in &xf.mod_weights[..=last] {
            out.push_str(&format!("{} ", g(*w)));
        }
        out.push_str("\" ");
    }

    out.push_str(&format!("opacity=\"{}\" ", g(xf.opacity)));
    if !xf.name.is_empty() {
        // NOTE the trailing space. The original omits it here, producing
        // `name="foo"var_color="1"` — malformed XML. We write valid output but
        // still READ the broken form; see xml.rs.
        out.push_str(&format!("name=\"{}\" ", esc(&xf.name)));
    }
    if xf.plugin_color != 1.0 {
        out.push_str(&format!("var_color=\"{}\" ", g(xf.plugin_color)));
    }

    out.push_str("/>\n");
    out
}

fn write_palette(f: &Flame) -> String {
    let mut out = String::from("   <palette count=\"256\" format=\"RGB\">");
    for (i, c) in f.palette.0.iter().take(256).enumerate() {
        if i % 8 == 0 {
            out.push_str("\n      ");
        }
        out.push_str(&format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
    }
    out.push_str("\n   </palette>\n");
    out
}

/// Serialise a batch as a complete `<flames>` document.
pub fn write_document(flames: &[Flame], title: &str) -> String {
    let mut out = format!("<flames name=\"{}\">\n", esc(title));
    for f in flames {
        out.push_str(&write_flame(f));
    }
    out.push_str("</flames>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;

    #[test]
    fn g_matches_delphi_percent_g() {
        assert_eq!(g(0.0), "0");
        assert_eq!(g(1.0), "1");
        assert_eq!(g(0.5), "0.5");
        assert_eq!(g(-0.25), "-0.25");
        // 6 significant digits, trailing zeros stripped.
        assert_eq!(g(1.0 / 3.0), "0.333333");
        assert_eq!(g(123456.0), "123456");
        // Beyond 6 significant digits switches to exponential.
        assert_eq!(g(1234567.0), "1.23457e+06");
        assert_eq!(g(0.000012345), "1.2345e-05");
    }

    fn sample() -> Flame {
        let doc = r#"<flame name="rt" size="640 480" center="0.25 -0.5" scale="42"
                            quality="120" brightness="3.5" gamma="2.5" vibrancy="0.8"
                            background="0 0.5 1" angle="0.25" new_linear="1">
            <xform weight="1.5" color="0.25" symmetry="0.5" julian="1" julian_power="5"
                   coefs="0.5 0.1 -0.1 0.5 0.2 0.3" opacity="0.75" chaos="0 2 1"
                   name="first"/>
            <xform weight="0.5" color="1" spherical="1" coefs="1 0 0 1 0 0"
                   post="2 0 0 2 1 1" opacity="1"/>
        </flame>"#;
        load::load(doc).flames.pop().unwrap()
    }

    /// The core guarantee: save then load reproduces the flame.
    #[test]
    fn round_trips_through_save_and_load() {
        let a = sample();
        let xml = write_flame(&a);
        let b = load::load(&xml).flames.pop().expect("reload failed");

        assert_eq!(a.name, b.name);
        assert_eq!((a.width, a.height), (b.width, b.height));
        assert!((a.center[0] - b.center[0]).abs() < 1e-6);
        assert!((a.center[1] - b.center[1]).abs() < 1e-6);
        assert!((a.pixels_per_unit - b.pixels_per_unit).abs() < 1e-6);
        assert!((a.sample_density - b.sample_density).abs() < 1e-6);
        assert!((a.brightness - b.brightness).abs() < 1e-6);
        assert!((a.gamma - b.gamma).abs() < 1e-6);
        assert!((a.vibrancy - b.vibrancy).abs() < 1e-6);
        assert!((a.angle - b.angle).abs() < 1e-6);

        assert_eq!(a.xforms.len(), b.xforms.len());
        for (x, y) in a.xforms.iter().zip(b.xforms.iter()) {
            assert!((x.density - y.density).abs() < 1e-6, "weight");
            assert!((x.color - y.color).abs() < 1e-6, "color");
            assert!((x.symmetry - y.symmetry).abs() < 1e-6, "symmetry");
            assert!((x.opacity - y.opacity).abs() < 1e-6, "opacity");
            assert!((x.coefs.a - y.coefs.a).abs() < 1e-6, "coefs");
            assert!((x.coefs.f - y.coefs.f).abs() < 1e-6, "coefs");
            assert_eq!(x.post.is_identity(), y.post.is_identity(), "post");
            assert_eq!(x.name, y.name, "name");
        }
    }

    #[test]
    fn round_trips_variation_parameters() {
        let a = sample();
        let b = load::load(&write_flame(&a)).flames.pop().unwrap();
        assert_eq!(b.xforms[0].variation_param("julian", "julian_power"), Some(5.0));
    }

    #[test]
    fn round_trips_xaos() {
        let a = sample();
        let b = load::load(&write_flame(&a)).flames.pop().unwrap();
        assert_eq!(b.xforms[0].mod_weights[0], 0.0);
        assert_eq!(b.xforms[0].mod_weights[1], 2.0);
        assert_eq!(b.xforms[0].mod_weights[2], 1.0);
    }

    /// The original drifts the background down one step per save/load cycle,
    /// because it writes bg/255 at 6 significant digits and reads back with
    /// floor(). Writing the bucket midpoint makes the round-trip exact.
    #[test]
    fn round_trips_background_through_normalisation() {
        let a = sample();
        let b = load::load(&write_flame(&a)).flames.pop().unwrap();
        assert_eq!(a.background, b.background, "background changed on round-trip");
    }

    /// And it must stay stable over many cycles, not merely one.
    #[test]
    fn background_does_not_drift_over_repeated_saves() {
        let mut f = sample();
        f.background = [7.0, 127.0, 248.0];
        let original = f.background;
        for i in 0..10 {
            f = load::load(&write_flame(&f)).flames.pop().unwrap();
            assert_eq!(f.background, original, "background drifted on cycle {i}");
        }
    }

    /// A value Apophysis itself wrote must still read the way Apophysis reads
    /// it, so our reader is unchanged and remains bug-compatible.
    #[test]
    fn reads_apophysis_written_background_unchanged() {
        // 0.498039 is what Apophysis emits for 127.
        let doc = r#"<flame name="a" size="10 10" background="0 0.498039 1" new_linear="1">
            <xform weight="1" linear="1"/>
        </flame>"#;
        let f = load::load(doc).flames.pop().unwrap();
        assert_eq!(f.background[1], 126.0, "reader must not be silently 'fixed'");
    }

    /// Output must always carry new_linear, or a reader will reconstruct the
    /// linear/flatten slots and change the render.
    #[test]
    fn always_writes_new_linear() {
        let xml = write_flame(&sample());
        assert!(xml.contains("new_linear=\"1\""), "new_linear missing from output");

        // And nothing gains a synthesised flatten on reload.
        let b = load::load(&xml).flames.pop().unwrap();
        let names: Vec<&str> = b.xforms[1].variations().iter().map(|(v, _)| v.name()).collect();
        assert!(!names.contains(&"flatten"), "flatten was synthesised: {names:?}");
    }

    /// Unlike the original, we emit the space after `name`.
    #[test]
    fn writes_valid_attribute_separators() {
        let mut f = sample();
        f.xforms[0].plugin_color = 0.5;
        let xml = write_flame(&f);
        assert!(!xml.contains("\"var_color"), "missing space before var_color: {xml}");
    }

    #[test]
    fn writes_a_batch_document() {
        let doc = write_document(&[sample(), sample()], "batch");
        assert!(doc.starts_with("<flames name=\"batch\">"));
        assert_eq!(load::load(&doc).flames.len(), 2);
    }
}
