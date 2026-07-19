// Ported from varLazysusan.pas, varLog.pas, varLoonie.pas, varMobius.pas,
// varNGon.pas, varpdj.pas, varPolar2.pas, varRectangles.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// All eight accumulate into FP and none touches the colour coordinate.
//
// Constants are copied digit-for-digit from the Pascal source rather than
// spelled as std::f64::consts — matching the original is the point.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use crate::plugins::is_zero;
use crate::variation::Pass;

variation! {
    /// `lazysusan`.
    ///
    /// Note the asymmetry: the input uses `ty + y` but both outputs subtract
    /// `y`. That is literal.
    Lazysusan, "lazysusan", Pass::Normal,
    params {
        // Wraps into [0, 2pi), writing the coerced value back.
        "lazysusan_spin" => spin = core::f64::consts::PI,
            coerce = |v: f64| (v / core::f64::consts::TAU).fract() * core::f64::consts::TAU,
            reset = core::f64::consts::PI,
        "lazysusan_space" => space = 0.0,
        "lazysusan_twist" => twist = 0.0,
        "lazysusan_x" => x = 0.0,
        // UPSTREAM BUG (UI only): the original's reset for lazysusan_y resets
        // lazysusan_x instead. Fixed here; cannot affect rendering.
        "lazysusan_y" => y = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let x = st.tx - s.x;
        let y = st.ty + s.y;
        let r = (x * x + y * y).sqrt();

        if r < s.w {
            let a = y.atan2(x) + s.spin + s.twist * (s.w - r);
            let (sina, cosa) = a.sin_cos();
            st.px += s.w * (r * cosa + s.x);
            st.py += s.w * (r * sina - s.y);
        } else {
            let r = 1.0 + s.space / (r + 1e-6);
            st.px += s.w * (r * x + s.x);
            st.py += s.w * (r * y - s.y);
        }
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `log`.
    Log, "log", Pass::Normal,
    params {
        // One-sided clamp, applied to the field only — the original does not
        // write it back. base == 1 gives ln(1) == 0 and divides by zero in
        // prepare; that is not guarded upstream either.
        "log_base" => base = 2.718_281_828_459_05,
            coerce = |v: f64| if v < 1e-6 { 1e-6 } else { v },
            reset = 2.718_281_828_459_05,
    }
    state { denom: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.denom = 0.5 / s.base.ln();
    }
    calc |s, st, _rng, _g| {
        // No guard on ln(0) at the origin.
        st.px += s.w * (st.tx * st.tx + st.ty * st.ty).ln() * s.denom;
        st.py += s.w * st.ty.atan2(st.tx);
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `loonie`.
    Loonie, "loonie", Pass::Normal,
    params {}
    state { sqrvar: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.sqrvar = w * w;
    }
    calc |s, st, _rng, _g| {
        let r2 = st.tx * st.tx + st.ty * st.ty;
        // Exact zero test, not an epsilon.
        if r2 < s.sqrvar && r2 != 0.0 {
            let r = s.w * (s.sqrvar / r2 - 1.0).sqrt();
            st.px += r * st.tx;
            st.py += r * st.ty;
        } else {
            st.px += s.w * st.tx;
            st.py += s.w * st.ty;
        }
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `mobius` — the Möbius transform (A*z + B) / (C*z + D) over the complex
    /// plane.
    ///
    /// The parameter names are bare and case-sensitive (`Re_A`, not
    /// `mobius_re_a`), which is unusual — every other variation prefixes.
    ///
    /// There is no guard on the denominator. With the default C = 0, D = 1 it
    /// is always 1, but a user can zero all four and divide by zero; the
    /// original relies on the resulting Inf/NaN being culled downstream.
    Mobius, "mobius", Pass::Normal,
    params {
        "Re_A" => re_a = 1.0, reset = 1.0,
        "Im_A" => im_a = 0.0,
        "Re_B" => re_b = 0.0,
        "Im_B" => im_b = 0.0,
        "Re_C" => re_c = 0.0,
        "Im_C" => im_c = 0.0,
        "Re_D" => re_d = 1.0, reset = 1.0,
        "Im_D" => im_d = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let (x, y) = (st.tx, st.ty);

        let u_re = s.re_a * x - s.im_a * y + s.re_b;
        let u_im = s.re_a * y + s.im_a * x + s.im_b;
        let v_re = s.re_c * x - s.im_c * y + s.re_d;
        let v_im = s.re_c * y + s.im_c * x + s.im_d;

        let denom = v_re * v_re + v_im * v_im;

        st.px += s.w * (u_re * v_re + u_im * v_im) / denom;
        st.py += s.w * (u_im * v_re - u_re * v_im) / denom;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `ngon`.
    Ngon, "ngon", Pass::Normal,
    params {
        // The open interval (-1, 1) is pushed out to -1 / +1 (0 goes to +1),
        // then rounded. Delphi's Round is banker's rounding.
        "ngon_sides" => sides = 4.0,
            coerce = |v: f64| {
                let v = if v < 0.0 && v > -1.0 {
                    -1.0
                } else if (0.0..1.0).contains(&v) {
                    1.0
                } else {
                    v
                };
                v.round_ties_even()
            },
            reset = 4.0,
        "ngon_power" => power = 2.0, reset = 2.0,
        "ngon_circle" => circle = 1.0, reset = 1.0,
        "ngon_corners" => corners = 1.0, reset = 1.0,
    }
    state { cpower: f64 = 0.0, csides: f64 = 0.0, csidesinv: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.cpower = -0.5 * s.power;
        s.csides = core::f64::consts::TAU / s.sides;
        s.csidesinv = 1.0 / s.csides;
    }
    calc |s, st, _rng, _g| {
        let (x, y) = (st.tx, st.ty);

        // Exact zero test on both coordinates.
        let r_factor = if x == 0.0 && y == 0.0 { 0.0 } else { (x * x + y * y).powf(s.cpower) };

        let theta = y.atan2(x);
        let mut phi = theta - s.csides * (theta * s.csidesinv).floor();
        if phi > 0.5 * s.csides {
            phi -= s.csides;
        }

        let amp = (s.corners * (1.0 / phi.cos() - 1.0) + s.circle) * s.w * r_factor;

        st.px += amp * x;
        st.py += amp * y;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `pdj`.
    ///
    /// The original randomises all four parameters in its constructor
    /// (uniform in [-3, 3)). A deterministic default is used instead; any
    /// `.flame` that specifies them overwrites it, and rendering is
    /// unaffected.
    Pdj, "pdj", Pass::Normal,
    params {
        "pdj_a" => a = 0.0,
        "pdj_b" => b = 0.0,
        "pdj_c" => c = 0.0,
        "pdj_d" => d = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        // Cross-coupled: a multiplies ty, b multiplies tx.
        st.px += s.w * ((s.a * st.ty).sin() - (s.b * st.tx).cos());
        st.py += s.w * ((s.c * st.tx).sin() - (s.d * st.ty).cos());
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `polar2`.
    ///
    /// `arctan2` has its arguments swapped here, and the original assigns y
    /// before x. No guard on ln(0).
    Polar2, "polar2", Pass::Normal,
    params {}
    state { p2vv: f64 = 0.0, p2vv2: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.p2vv = w / core::f64::consts::PI;
        s.p2vv2 = s.p2vv * 0.5;
    }
    calc |s, st, _rng, _g| {
        st.py += s.p2vv2 * (st.tx * st.tx + st.ty * st.ty).ln();
        st.px += s.p2vv * st.tx.atan2(st.ty);
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `rectangles`.
    ///
    /// Dispatches on `IsZero` per axis — an epsilon compare, so
    /// `rectangles_x = 1e-13` passes x through instead of dividing by it.
    Rectangles, "rectangles", Pass::Normal,
    params {
        "rectangles_x" => x = 1.0, reset = 1.0,
        "rectangles_y" => y = 1.0, reset = 1.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        st.px += if is_zero(s.x) {
            s.w * st.tx
        } else {
            s.w * ((2.0 * (st.tx / s.x).floor() + 1.0) * s.x - st.tx)
        };
        st.py += if is_zero(s.y) {
            s.w * st.ty
        } else {
            s.w * ((2.0 * (st.ty / s.y).floor() + 1.0) * s.y - st.ty)
        };
        st.pz += s.w * st.tz;
    }
}

pub const NAMES: [&str; 8] =
    ["lazysusan", "log", "loonie", "mobius", "ngon", "pdj", "polar2", "rectangles"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "lazysusan" => Some(Box::new(Lazysusan::default())),
        "log" => Some(Box::new(Log::default())),
        "loonie" => Some(Box::new(Loonie::default())),
        "mobius" => Some(Box::new(Mobius::default())),
        "ngon" => Some(Box::new(Ngon::default())),
        "pdj" => Some(Box::new(Pdj::default())),
        "polar2" => Some(Box::new(Polar2::default())),
        "rectangles" => Some(Box::new(Rectangles::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::{VarState, Variation};

    /// Default Mobius parameters are the identity transform.
    #[test]
    fn mobius_default_is_identity() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Mobius::default();
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { tx: 0.3, ty: -0.4, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert!((st.px - 0.3).abs() < 1e-12, "px: {}", st.px);
        assert!((st.py + 0.4).abs() < 1e-12, "py: {}", st.py);
    }

    #[test]
    fn ngon_sides_coercion_pushes_out_of_the_open_unit_interval() {
        let mut v = Ngon::default();
        assert_eq!(v.set_param("ngon_sides", 0.0), Some(1.0));
        assert_eq!(v.set_param("ngon_sides", 0.5), Some(1.0));
        assert_eq!(v.set_param("ngon_sides", -0.5), Some(-1.0));
        assert_eq!(v.set_param("ngon_sides", 6.4), Some(6.0));
        // Banker's rounding: 2.5 goes to 2, not 3.
        assert_eq!(v.set_param("ngon_sides", 2.5), Some(2.0));
    }

    /// rectangles must use an epsilon compare, not `== 0`.
    #[test]
    fn rectangles_uses_epsilon_zero_test() {
        let mut rng = Rng::new(2);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Rectangles::default();
        v.x = 1e-13;
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { tx: 0.4, ty: 0.0, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert!((st.px - 0.4).abs() < 1e-12, "tiny x should pass through: {}", st.px);
    }

    #[test]
    fn lazysusan_spin_wraps_into_a_turn() {
        let mut v = Lazysusan::default();
        let got = v.set_param("lazysusan_spin", 3.0 * core::f64::consts::TAU + 1.0).unwrap();
        assert!(got.abs() < core::f64::consts::TAU, "spin not wrapped: {got}");
    }
}
