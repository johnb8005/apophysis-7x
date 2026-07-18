// Ported from varAuger.pas, varBipolar.pas, varCross.pas, varElliptic.pas,
// varEpispiral.pas, varEscher.pas, varFan2.pas, varFoci.pas, varHemisphere.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// All nine accumulate into FP. None touches the colour coordinate.

use crate::variation::Pass;

variation! {
    /// `auger`. Note the cross-coupling: dx is driven by sin(freq*y) and dy by
    /// sin(freq*x).
    Auger, "auger", Pass::Normal,
    params {
        "auger_freq" => freq = 5.0, reset = 5.0,
        "auger_weight" => weight = 0.5, reset = 0.5,
        // UPSTREAM BUG (UI only): the original's ResetVariable for
        // auger_scale assigns auger_sym instead, so resetting scale does
        // nothing. Fixed here — it cannot affect rendering.
        "auger_scale" => scale = 0.1, reset = 0.1,
        "auger_sym" => sym = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let (x, y) = (st.tx, st.ty);
        let sx = (s.freq * x).sin();
        let ty = (s.freq * y).sin();

        let dx = x + s.weight * (0.5 * s.scale * ty + x.abs() * ty);
        let dy = y + s.weight * (0.5 * s.scale * sx + y.abs() * sx);

        st.px += s.w * (x + s.sym * (dx - x));
        st.py += s.w * dy;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `bipolar`.
    ///
    /// The degenerate branch returns early WITHOUT accumulating z — the
    /// original's `Exit` sits above its `FPz` line.
    Bipolar, "bipolar", Pass::Normal,
    params {
        // Wraps into (-1, 1], writing the coerced value back.
        "bipolar_shift" => shift = 0.0,
            coerce = |v: f64| 2.0 * (0.5 * (v + 1.0)).fract() - 1.0,
    }
    state { v4: f64 = 0.0, v: f64 = 0.0, s: f64 = 0.0 }
    prepare |st, w, _c, _rng| {
        st.v4 = w * 0.159_154_943_091_895_335_768_883_763_372_51;
        st.v = w * 0.636_619_772_367_581_343_075_535_053_490_061;
        st.s = -1.570_796_326_794_896_619_23 * st.shift;
    }
    calc |s, st, _rng, _g| {
        const HALF_PI: f64 = 1.570_796_326_794_896_619_23;
        let x2y2 = st.tx * st.tx + st.ty * st.ty;
        let mut y = 0.5 * (2.0 * st.ty).atan2(x2y2 - 1.0) + s.s;

        if y > HALF_PI {
            y = -HALF_PI + (y + HALF_PI) % core::f64::consts::PI;
        } else if y < -HALF_PI {
            y = HALF_PI - (HALF_PI - y) % core::f64::consts::PI;
        }

        let t = x2y2 + 1.0;
        let x2 = 2.0 * st.tx;
        let f = t + x2;
        let g2 = t - x2;

        if g2 == 0.0 || f / g2 <= 0.0 {
            return;
        }

        st.px += s.v4 * ((t + x2) / (t - x2)).ln();
        st.py += s.v * y;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `cross`.
    ///
    /// The 1e-6 sits INSIDE the `Abs`, so it does not actually guarantee a
    /// nonzero denominator — `(x-y)(x+y)` can be exactly -1e-6.
    Cross, "cross", Pass::Normal,
    params {}
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let (x, y) = (st.tx, st.ty);
        let r = ((x - y) * (x + y) + 1e-6).abs();
        let r = s.w / r;
        st.px += x * r;
        st.py += y * r;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `elliptic`.
    Elliptic, "elliptic", Pass::Normal,
    params {}
    state { v: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.v = w / core::f64::consts::FRAC_PI_2;
    }
    calc |s, st, _rng, _g| {
        // Only the two `sqrt_safe` call sites clamp; the tmp +- x2 roots do not.
        let sqrt_safe = |x: f64| if x < 0.0 { 0.0 } else { x.sqrt() };

        let (x, y) = (st.tx, st.ty);
        let tmp = y * y + x * x + 1.0;
        let x2 = 2.0 * x;
        let xmax = 0.5 * ((tmp + x2).sqrt() + (tmp - x2).sqrt());

        let a = x / xmax;
        let b = sqrt_safe(1.0 - a * a);

        // z is accumulated before x and y in the original.
        st.pz += s.w * st.tz;
        st.px += s.v * a.atan2(b);

        // y == 0 takes the negative branch (test is `> 0`).
        let term = s.v * (xmax + sqrt_safe(xmax - 1.0)).ln();
        if y > 0.0 {
            st.py += term;
        } else {
            st.py -= term;
        }
    }
}

variation! {
    /// `epispiral`.
    ///
    /// Never touches z — there is no FPz line in the original. The RNG draw is
    /// unconditional even when thickness is 0, so the stream advances once per
    /// call regardless.
    Epispiral, "epispiral", Pass::Normal,
    params {
        "epispiral_n" => n = 6.0, reset = 6.0,
        "epispiral_thickness" => thickness = 0.0,
        // Constructor default is 1.0 but reset goes to 0.0 — not a typo.
        "epispiral_holes" => holes = 1.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, rng, _g| {
        let theta = st.ty.atan2(st.tx);
        let t = (rng.f64() * s.thickness) * (1.0 / (s.n * theta).cos()) - s.holes;

        if t.abs() == 0.0 {
            return;
        }
        st.px += s.w * t * theta.cos();
        st.py += s.w * t * theta.sin();
    }
}

variation! {
    /// `escher`.
    Escher, "escher", Pass::Normal,
    params {
        // Wraps beta into [-PI, PI), writing the coerced value back.
        "escher_beta" => beta = 0.0,
            coerce = |v: f64| {
                ((v + core::f64::consts::PI) / core::f64::consts::TAU).fract()
                    * core::f64::consts::TAU
                    - core::f64::consts::PI
            },
    }
    state { c: f64 = 0.0, d: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (sin_b, cos_b) = s.beta.sin_cos();
        s.c = 0.5 * (1.0 + cos_b);
        s.d = 0.5 * sin_b;
    }
    calc |s, st, _rng, _g| {
        let a = st.ty.atan2(st.tx);
        // No guard on ln(0) at the origin.
        let lnr = 0.5 * (st.tx * st.tx + st.ty * st.ty).ln();

        let m = s.w * (s.c * lnr - s.d * a).exp();
        let (sn, cs) = (s.c * a + s.d * lnr).sin_cos();

        st.px += m * cs;
        st.py += m * sn;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `fan2`.
    ///
    /// Three literal traps: `arctan2(x, y)` has its arguments swapped;
    /// `System.Frac` truncates and keeps its sign, so `> 0.5` is never true
    /// for negative input; and the output is cos->x, sin->y.
    Fan2, "fan2", Pass::Normal,
    params {
        // The original randomises both in its constructor; a deterministic
        // default is used instead, and any .flame overwrites it anyway.
        "fan2_x" => x = 0.0,
        "fan2_y" => y = 0.0,
    }
    state { dx: f64 = 0.0, dy: f64 = 0.0, dx2: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        const EPS: f64 = 1e-10;
        s.dy = s.y;
        s.dx = core::f64::consts::PI * (s.x * s.x + EPS);
        s.dx2 = s.dx / 2.0;
    }
    calc |s, st, _rng, _g| {
        let angle = st.tx.atan2(st.ty);
        // Truncating fract, matching System.Frac.
        let a = if ((angle + s.dy) / s.dx).fract() > 0.5 {
            angle - s.dx2
        } else {
            angle + s.dx2
        };
        let (sinr, cosr) = a.sin_cos();
        let r = s.w * (st.tx * st.tx + st.ty * st.ty).sqrt();
        st.px += r * cosr;
        st.py += r * sinr;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `foci`.
    Foci, "foci", Pass::Normal,
    params {}
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let expx = st.tx.exp() * 0.5;
        // Kept as a division rather than 0.5*exp(-x) for bit-exactness.
        let expnx = 0.25 / expx;
        let (siny, cosy) = st.ty.sin_cos();

        let mut tmp = expx + expnx - cosy;
        // Exact zero test replaced by 1e-6, not an epsilon band.
        if tmp == 0.0 {
            tmp = 1e-6;
        }
        let tmp = s.w / tmp;

        st.px += (expx - expnx) * tmp;
        st.py += siny * tmp;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `hemisphere`.
    ///
    /// The only variation here whose z output is not `w * tz` — it ignores the
    /// incoming z entirely and writes the sphere term.
    Hemisphere, "hemisphere", Pass::Normal,
    params {}
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let t = s.w / (st.tx * st.tx + st.ty * st.ty + 1.0).sqrt();
        st.px += st.tx * t;
        st.py += st.ty * t;
        st.pz += t;
    }
}

pub const NAMES: [&str; 9] = [
    "auger",
    "bipolar",
    "cross",
    "elliptic",
    "epispiral",
    "escher",
    "fan2",
    "foci",
    "hemisphere",
];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "auger" => Some(Box::new(Auger::default())),
        "bipolar" => Some(Box::new(Bipolar::default())),
        "cross" => Some(Box::new(Cross::default())),
        "elliptic" => Some(Box::new(Elliptic::default())),
        "epispiral" => Some(Box::new(Epispiral::default())),
        "escher" => Some(Box::new(Escher::default())),
        "fan2" => Some(Box::new(Fan2::default())),
        "foci" => Some(Box::new(Foci::default())),
        "hemisphere" => Some(Box::new(Hemisphere::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::{VarState, Variation};

    /// bipolar's degenerate branch must skip z too.
    #[test]
    fn bipolar_degenerate_branch_skips_z() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Bipolar::default();
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        // x = 0, y = 0 gives t = 1, x2 = 0, so f = g = 1 and f/g = 1 > 0:
        // not degenerate. Use a point where t - x2 == 0, i.e. x2y2 + 1 == 2x.
        // x = 1, y = 0: t = 2, x2 = 2, g = 0 -> degenerate.
        let mut st = VarState { tx: 1.0, ty: 0.0, tz: 5.0, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert_eq!(st.pz, 0.0, "degenerate branch must not accumulate z");
        assert_eq!(st.px, 0.0);
    }

    /// hemisphere ignores incoming z rather than scaling it.
    #[test]
    fn hemisphere_ignores_incoming_z() {
        let mut rng = Rng::new(2);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Hemisphere::default();
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut a = VarState { tx: 0.5, ty: 0.5, tz: 0.0, ..Default::default() };
        let mut b = VarState { tx: 0.5, ty: 0.5, tz: 99.0, ..Default::default() };
        v.calc(&mut a, &mut rng, &mut g);
        v.calc(&mut b, &mut rng, &mut g);
        assert_eq!(a.pz, b.pz, "z output must not depend on input z");
    }

    /// epispiral never writes z at all.
    #[test]
    fn epispiral_never_touches_z() {
        let mut rng = Rng::new(3);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Epispiral::default();
        v.thickness = 1.0;
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { tx: 0.4, ty: 0.6, tz: 7.0, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert_eq!(st.pz, 0.0, "epispiral has no FPz line");
    }

    #[test]
    fn shift_and_beta_wrap_on_set() {
        let mut b = Bipolar::default();
        let got = b.set_param("bipolar_shift", 3.0).unwrap();
        assert!((-1.0..=1.0).contains(&got), "shift not wrapped: {got}");

        let mut e = Escher::default();
        let got = e.set_param("escher_beta", 10.0).unwrap();
        assert!(
            got >= -core::f64::consts::PI && got < core::f64::consts::PI,
            "beta not wrapped: {got}"
        );
    }
}
