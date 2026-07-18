// Tone curves — ported from `src/Core/Bezier.pas` and the LUT construction in
// `TImageMaker.CreateImage` (ImageMaker.pas:454-464).
// See LICENSE (GPL-2.0-or-later) at the repo root.

/// One rational cubic Bézier curve: four control points and their weights.
///
/// Channel order matches the original's `curvePoints[0..3]`: overall, red,
/// green, blue. The overall curve is applied first and its output feeds the
/// per-channel curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub points: [(f64, f64); 4],
    pub weights: [f64; 4],
}

impl Default for Curve {
    fn default() -> Self {
        // The identity curve the original treats as "unset".
        Curve {
            points: [(0.0, 0.0), (0.0, 0.0), (1.0, 1.0), (1.0, 1.0)],
            weights: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl Curve {
    pub fn is_identity(&self) -> bool {
        *self == Curve::default()
    }

    /// Evaluate the curve at parameter `t`, returning y.
    ///
    /// NOTE this is `y(t)`, not `y(x)` — the LUT samples uniformly in the
    /// Bézier *parameter*, so the control points' x-coordinates never affect
    /// the result. That is what the original does (`BezierFunc(i/256, ...)`),
    /// and "fixing" it to solve for t given x would change every rendered
    /// image that uses curves.
    pub fn eval(&self, t: f64) -> f64 {
        let s = 1.0 - t;
        let (s2, s3) = (s * s, s * s * s);
        let (t2, t3) = (t * t, t * t * t);
        let w = &self.weights;
        let p = &self.points;

        let nom_y = w[0] * s3 * p[0].1
            + w[1] * s2 * 3.0 * t * p[1].1
            + w[2] * s * 3.0 * t2 * p[2].1
            + w[3] * t3 * p[3].1;
        let denom = w[0] * s3 + w[1] * s2 * 3.0 * t + w[2] * s * 3.0 * t2 + w[3] * t3;

        // The original bails out here leaving its out-parameter untouched,
        // which in Delphi means an uninitialised local. We fall back to the
        // identity instead of propagating garbage.
        if !nom_y.is_finite() || !denom.is_finite() || denom == 0.0 {
            return t;
        }
        nom_y / denom
    }
}

/// The four curves of a flame, plus their sampled lookup tables.
#[derive(Clone, Debug)]
pub struct Curves {
    /// Overall, red, green, blue.
    pub channels: [Curve; 4],
}

impl Default for Curves {
    fn default() -> Self {
        Curves { channels: [Curve::default(); 4] }
    }
}

/// A 257-entry lookup per channel, indexed by a 0..256 tone value.
pub struct CurveLut {
    tables: [[f64; 257]; 4],
    active: bool,
}

impl Curves {
    /// True when any curve differs from the identity. When false the tone
    /// mapper skips the lookup entirely, as the original's `curvesSet` does.
    pub fn is_active(&self) -> bool {
        self.channels.iter().any(|c| !c.is_identity())
    }

    pub fn build_lut(&self) -> CurveLut {
        let mut tables = [[0.0f64; 257]; 4];
        for (n, curve) in self.channels.iter().enumerate() {
            for i in 0..=256 {
                tables[n][i] = curve.eval(i as f64 / 256.0) * 256.0;
            }
        }
        CurveLut { tables, active: self.is_active() }
    }
}

impl CurveLut {
    pub fn active(&self) -> bool {
        self.active
    }

    /// Apply the overall curve then the channel curve, as
    /// `csa[1 + ch][ round(csa[0][v]) ]` does.
    ///
    /// `channel` is 0=red, 1=green, 2=blue.
    #[inline]
    pub fn apply(&self, channel: usize, value: f64) -> f64 {
        if !self.active {
            return value;
        }
        // The original guards with `(ri >= 0) and (ri <= 256)`, leaving values
        // outside that window untouched rather than clamping them.
        if !(0.0..=256.0).contains(&value) {
            return value;
        }
        let master = self.tables[0][value.round() as usize % 257];
        if !(0.0..=256.0).contains(&master) {
            return master;
        }
        self.tables[1 + channel][master.round() as usize % 257]
    }
}

/// Parse the `curves` attribute: 4 channels x 4 points x (x, y, weight).
pub fn parse(values: &[f64]) -> Option<Curves> {
    if values.len() < 48 {
        return None;
    }
    let mut curves = Curves::default();
    for ch in 0..4 {
        for p in 0..4 {
            let base = ch * 12 + p * 3;
            curves.channels[ch].points[p] = (values[base], values[base + 1]);
            curves.channels[ch].weights[p] = values[base + 2];
        }
    }
    Some(curves)
}

/// Serialise back to the same 48-value layout.
pub fn to_values(curves: &Curves) -> Vec<f64> {
    let mut out = Vec::with_capacity(48);
    for ch in 0..4 {
        for p in 0..4 {
            out.push(curves.channels[ch].points[p].0);
            out.push(curves.channels[ch].points[p].1);
            out.push(curves.channels[ch].weights[p]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_curve_is_detected() {
        let c = Curves::default();
        assert!(!c.is_active(), "default curves must be treated as unset");
        assert!(!c.build_lut().active());
    }

    /// The default control points are NOT the linear ramp: (0,0),(0,0),(1,1),
    /// (1,1) evaluates to 3t^2 - 2t^3, i.e. smoothstep. That only stays
    /// harmless because the whole lookup is skipped when every curve is at the
    /// default — enabling curves with untouched points would silently apply an
    /// S-curve to the image.
    #[test]
    fn default_curve_is_smoothstep_not_a_ramp() {
        let c = Curve::default();
        for i in [0, 64, 128, 200, 256] {
            let t = i as f64 / 256.0;
            let expected = 3.0 * t * t - 2.0 * t * t * t;
            assert!((c.eval(t) - expected).abs() < 1e-9, "eval({t}) = {}", c.eval(t));
        }
        // Endpoints and the midpoint still land where you would expect.
        assert!((c.eval(0.0) - 0.0).abs() < 1e-12);
        assert!((c.eval(0.5) - 0.5).abs() < 1e-12);
        assert!((c.eval(1.0) - 1.0).abs() < 1e-12);

        // The safety net: it is never actually applied.
        assert!(!Curves::default().is_active());
    }

    #[test]
    fn inactive_lut_passes_values_through() {
        let lut = Curves::default().build_lut();
        for v in [0.0, 55.0, 128.0, 255.0] {
            assert_eq!(lut.apply(0, v), v);
        }
    }

    /// A curve pulled toward the top should brighten mid-tones.
    #[test]
    fn raised_curve_brightens() {
        let mut c = Curves::default();
        c.channels[0].points[1] = (0.25, 0.6);
        c.channels[0].points[2] = (0.75, 0.95);
        assert!(c.is_active());

        let lut = c.build_lut();
        let out = lut.apply(0, 128.0);
        assert!(out > 128.0, "expected brightening, got {out}");
    }

    #[test]
    fn round_trips_through_the_48_value_layout() {
        let mut c = Curves::default();
        c.channels[2].points[1] = (0.3, 0.7);
        c.channels[2].weights[1] = 2.0;

        let values = to_values(&c);
        assert_eq!(values.len(), 48);
        let back = parse(&values).expect("parse failed");
        assert_eq!(back.channels[2].points[1], (0.3, 0.7));
        assert_eq!(back.channels[2].weights[1], 2.0);
    }

    #[test]
    fn parse_rejects_short_input() {
        assert!(parse(&[0.0; 12]).is_none());
    }

    /// A zero denominator must fall back to the identity rather than
    /// returning uninitialised memory, as the original effectively does.
    #[test]
    fn degenerate_weights_fall_back_to_identity() {
        let c = Curve { points: [(0.0, 0.0); 4], weights: [0.0; 4] };
        for t in [0.0, 0.5, 1.0] {
            assert_eq!(c.eval(t), t, "expected identity fallback at {t}");
        }
    }
}

#[cfg(test)]
mod render_integration {
    use super::*;
    use crate::builtins::{Builtin, BuiltinVar};
    use crate::flame::{Affine, XForm};
    use crate::genome::Flame;
    use crate::rng::Rng;
    use crate::variation::Variation;

    fn tiny_flame() -> Flame {
        let mut f = Flame::default();
        let half = |e: f64, ff: f64| Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e, f: ff };
        for (i, c) in [(half(0.0, 0.0), 0.0), (half(0.5, 0.0), 0.5), (half(0.25, 0.5), 1.0)]
            .into_iter()
            .enumerate()
        {
            let mut xf = XForm::default();
            xf.coefs = c.0;
            xf.color = c.1;
            xf.density = 1.0;
            let _ = i;
            xf.set_variations(vec![(
                Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
                1.0,
            )]);
            f.xforms.push(xf);
        }
        f.width = 96;
        f.height = 96;
        f.center = [0.5, 0.5];
        f.pixels_per_unit = 96.0;
        f.sample_density = 40.0;
        f
    }

    fn mean_luma(f: &Flame) -> f64 {
        let mut f = f.clone();
        let mut rng = Rng::new(1);
        f.prepare(&mut rng);
        let img = crate::render::render(&f, 1);
        let sum: u64 = img
            .data
            .chunks(4)
            .map(|p| (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3)
            .sum();
        sum as f64 / (img.width * img.height) as f64
    }

    /// The whole point of the feature: a raised curve must brighten the render.
    #[test]
    fn curves_change_the_rendered_image() {
        let base = tiny_flame();
        let plain = mean_luma(&base);

        let mut curved = base.clone();
        // Pull the overall curve well above the diagonal.
        curved.curves.channels[0].points[1] = (0.25, 0.85);
        curved.curves.channels[0].points[2] = (0.75, 0.98);
        assert!(curved.curves.is_active(), "curve should register as active");

        let lifted = mean_luma(&curved);
        assert!(
            (lifted - plain).abs() > 0.5,
            "curve had no effect on the render: {plain} vs {lifted}"
        );
        assert!(lifted > plain, "raised curve should brighten: {plain} -> {lifted}");
    }
}
