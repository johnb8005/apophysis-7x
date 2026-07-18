// Ported from varRadialBlur.pas, varBlurCircle.pas, varBlurZoom.pas,
// varBlurPixelize.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::plugins::is_zero;
use crate::variation::Pass;

variation! {
    /// `radial_blur`.
    ///
    /// The original keeps its OWN 4-element uniform ring buffer, seeded in
    /// `Prepare` and advanced once per call, summing to `[-2, 2)`. That is the
    /// same Irwin-Hall scheme as the built-in gaussian blurs, so we reuse the
    /// shared `GaussBuf` rather than adding per-instance mutable state behind
    /// the `&self` calc signature. The distribution and the tap-to-tap
    /// correlation are identical; only the interleaving with other blur
    /// variations in the same xform differs.
    RadialBlur, "radial_blur", Pass::Normal,
    params {
        // Reset is "0 if nonzero, else 1" in the original — not a constant.
        "radial_blur_angle" => angle = 0.0,
            reset_fn = |cur: f64| if cur != 0.0 { 0.0 } else { 1.0 },
    }
    state { spin: f64 = 0.0, zoom: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.spin = w * (s.angle * core::f64::consts::FRAC_PI_2).sin();
        s.zoom = w * (s.angle * core::f64::consts::FRAC_PI_2).cos();
    }
    calc |s, st, rng, g| {
        let (x, y) = (st.tx, st.ty);
        let rnd_g = g.next(rng);

        // Dispatch mirrors GetCalcFunction: IsZero is an epsilon compare.
        if is_zero(s.spin) {
            // CalcZoom
            let r = s.zoom * rnd_g;
            st.px += r * x;
            st.py += r * y;
            st.pz += s.w * st.tz;
            return;
        }
        if is_zero(s.zoom) {
            // CalcSpin
            let (sina, cosa) = (y.atan2(x) + s.spin * rnd_g).sin_cos();
            let r = (x * x + y * y).sqrt();
            st.px += r * cosa - x;
            st.py += r * sina - y;
            st.pz += s.w * st.tz;
            return;
        }

        let ra = (x * x + y * y).sqrt();
        let (sina, cosa) = (y.atan2(x) + s.spin * rnd_g).sin_cos();
        let rz = s.zoom * rnd_g - 1.0;
        st.px += ra * cosa + rz * x;
        st.py += ra * sina + rz * y;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `blur_circle` — a pure blur; x and y outputs ignore the input entirely.
    ///
    /// The square-to-circle mapping walks the perimeter of a unit square and
    /// remaps it onto a circle. Note the mixed signed/absolute comparisons
    /// (`x >= absy`, `y >= absx`) — those are literal.
    BlurCircle, "blur_circle", Pass::Normal,
    params {}
    state { pi_4: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.pi_4 = core::f64::consts::FRAC_PI_4;
        // The original also computes VVAR*4/PI here and never uses it.
    }
    calc |s, st, rng, _g| {
        let x = 2.0 * rng.f64() - 1.0;
        let y = 2.0 * rng.f64() - 1.0;

        let absx = x.abs();
        let absy = y.abs();

        let (perimeter, side) = if absx >= absy {
            if x >= absy {
                (absx + y, absx)
            } else {
                (5.0 * absx - y, absx)
            }
        } else if y >= absx {
            (3.0 * absy - x, absy)
        } else {
            (7.0 * absy + x, absy)
        };

        let r = s.w * side;
        let (sina, cosa) = (s.pi_4 * perimeter / side - s.pi_4).sin_cos();

        st.px += r * cosa;
        st.py += r * sina;
        st.pz += s.w * st.tz;
    }
}

/// `blur_zoom`.
///
/// UPSTREAM BUG, REPRODUCED DELIBERATELY — hand-written rather than generated
/// by `variation!` because the defect is in the setter itself and the macro
/// cannot express a cross-field assignment.
///
/// The original's `SetVariable` assigns `blur_zoom_y` for BOTH parameters:
///
/// ```text
/// end else if Name = 'blur_zoom_x' then begin
///   blur_zoom_y := Value;      // <-- not blur_zoom_x
///   Result := True;
/// end else if Name = 'blur_zoom_y' then begin
///   blur_zoom_y := Value;
/// ```
///
/// So `blur_zoom_x` can never be set from a `.flame` file: it stays 0 forever,
/// and writing x silently clobbers y. `GetVariable` reads the correct field,
/// making the round-trip asymmetric.
///
/// Fixing this would make every existing flame using `blur_zoom` render
/// differently here than in Apophysis, which defeats the point of the port —
/// so the defect is preserved exactly. See the test below, which pins it.
#[derive(Clone, Default)]
pub struct BlurZoom {
    pub w: f64,
    pub length: f64,
    /// Always 0 in practice: the setter can never write it. Kept so the
    /// editor can display the parameter and so `get_param` matches Apophysis's
    /// (correct) getter.
    pub x: f64,
    pub y: f64,
}

impl crate::variation::Variation for BlurZoom {
    fn name(&self) -> &'static str {
        "blur_zoom"
    }

    fn pass(&self) -> Pass {
        Pass::Normal
    }

    fn param_names(&self) -> &'static [&'static str] {
        &["blur_zoom_length", "blur_zoom_x", "blur_zoom_y"]
    }

    fn get_param(&self, name: &str) -> Option<f64> {
        match name {
            "blur_zoom_length" => Some(self.length),
            "blur_zoom_x" => Some(self.x),
            "blur_zoom_y" => Some(self.y),
            _ => None,
        }
    }

    fn set_param(&mut self, name: &str, value: f64) -> Option<f64> {
        match name {
            "blur_zoom_length" => {
                self.length = value;
                Some(value)
            }
            // The bug: x writes y.
            "blur_zoom_x" => {
                self.y = value;
                Some(value)
            }
            "blur_zoom_y" => {
                self.y = value;
                Some(value)
            }
            _ => None,
        }
    }

    fn prepare(&mut self, weight: f64, _coefs: &crate::flame::Affine, _rng: &mut crate::rng::Rng) {
        self.w = weight;
    }

    #[inline(always)]
    fn calc(
        &self,
        st: &mut crate::variation::VarState,
        rng: &mut crate::rng::Rng,
        _g: &mut crate::rng::GaussBuf,
    ) {
        let z = 1.0 + self.length * rng.f64();
        // Also literal: the x term re-adds `+ x`, the y term subtracts `- y`.
        st.px += self.w * ((st.tx - self.x) * z + self.x);
        st.py += self.w * ((st.ty - self.y) * z - self.y);
        st.pz += self.w * st.tz;
    }
}

variation! {
    /// `blur_pixelize` — snaps to a grid, then jitters within the cell.
    BlurPixelize, "blur_pixelize", Pass::Normal,
    params {
        "blur_pixelize_size" => size = 0.1,
            coerce = |v: f64| if v < 1e-6 { 1e-6 } else { v },
            reset = 0.1,
        "blur_pixelize_scale" => scale = 1.0, reset = 1.0,
    }
    state { inv_size: f64 = 0.0, v: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.inv_size = 1.0 / s.size;
        s.v = w * s.size;
    }
    calc |s, st, rng, _g| {
        let x = (st.tx * s.inv_size).floor();
        let y = (st.ty * s.inv_size).floor();
        st.px += s.v * (x + s.scale * (rng.f64() - 0.5) + 0.5);
        st.py += s.v * (y + s.scale * (rng.f64() - 0.5) + 0.5);
        st.pz += s.w * st.tz;
    }
}

pub const NAMES: [&str; 4] = ["radial_blur", "blur_circle", "blur_zoom", "blur_pixelize"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "radial_blur" => Some(Box::new(RadialBlur::default())),
        "blur_circle" => Some(Box::new(BlurCircle::default())),
        "blur_zoom" => Some(Box::new(BlurZoom::default())),
        "blur_pixelize" => Some(Box::new(BlurPixelize::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variation::Variation;

    /// Pins the reproduced upstream bug: setting blur_zoom_x writes y and
    /// leaves x at 0. If someone "fixes" this, renders of existing flames
    /// silently diverge from Apophysis.
    #[test]
    fn blur_zoom_x_is_unsettable_as_in_original() {
        let mut v = BlurZoom::default();
        v.set_param("blur_zoom_x", 0.75);
        assert_eq!(v.x, 0.0, "blur_zoom_x must remain unset");
        assert_eq!(v.y, 0.75, "writing x must clobber y, as upstream does");

        v.set_param("blur_zoom_y", 0.25);
        assert_eq!(v.y, 0.25);
        assert_eq!(v.x, 0.0);
    }

    #[test]
    fn blur_pixelize_size_has_a_floor() {
        let mut v = BlurPixelize::default();
        assert_eq!(v.set_param("blur_pixelize_size", -3.0), Some(1e-6));
        assert_eq!(v.set_param("blur_pixelize_size", 0.0), Some(1e-6));
        assert_eq!(v.set_param("blur_pixelize_size", 0.25), Some(0.25));
    }

    /// radial_blur's reset is "0 if set, else 1" rather than a constant.
    #[test]
    fn radial_blur_reset_toggles() {
        let mut v = RadialBlur::default();
        v.set_param("radial_blur_angle", 0.5);
        assert_eq!(v.reset_param("radial_blur_angle"), Some(0.0));
        assert_eq!(v.reset_param("radial_blur_angle"), Some(1.0));
    }

    /// blur_circle ignores its x/y input entirely — only z passes through.
    #[test]
    fn blur_circle_ignores_input_position() {
        use crate::flame::Affine;
        use crate::rng::{GaussBuf, Rng};
        use crate::variation::VarState;

        let mut v = BlurCircle::default();
        let mut rng = Rng::new(1);
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);
        let mut g = GaussBuf::new(&mut rng);

        let mut a = VarState { tx: 0.0, ty: 0.0, tz: 1.0, ..Default::default() };
        let mut b = VarState { tx: 99.0, ty: -99.0, tz: 1.0, ..Default::default() };
        v.calc(&mut a, &mut Rng::new(5), &mut g);
        v.calc(&mut b, &mut Rng::new(5), &mut g);

        assert_eq!(a.px, b.px, "output must not depend on input x");
        assert_eq!(a.py, b.py, "output must not depend on input y");
    }
}
