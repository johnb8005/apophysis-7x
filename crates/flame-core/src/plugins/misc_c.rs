// Ported from varRings2.pas, varScry.pas, varSeparation.pas, varSplits.pas,
// varWaves2.pas, varWedge.pas, varPreDisc.pas, varPreSinusoidal.pas,
// varPreSpherical.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// The last three are pre_ variations: they overwrite FT in place and never
// touch FP. None of the nine touches the colour coordinate.

use crate::variation::Pass;

variation! {
    /// `rings2`.
    ///
    /// `System.Int` truncates toward zero — it is NOT floor. There is no guard
    /// on `Length == 0`, so `2/Length` can divide by zero.
    Rings2, "rings2", Pass::Normal,
    params {
        // The original seeds this with `Random * 2` per instance; a
        // deterministic default is used instead.
        "rings2_val" => val = 0.0,
    }
    state { dx: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        const EPS: f64 = 1e-10;
        s.dx = s.val * s.val + EPS;
    }
    calc |s, st, _rng, _g| {
        let len = (st.tx * st.tx + st.ty * st.ty).sqrt();
        let r = s.w * (2.0 - s.dx * (((len / s.dx + 1.0) / 2.0).trunc() * 2.0 / len + 1.0));
        st.px += r * st.tx;
        st.py += r * st.ty;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `scry`.
    ///
    /// The weight enters only through `v`; the x/y outputs are not multiplied
    /// by it. No epsilon guard on `sqrt(t)`.
    Scry, "scry", Pass::Normal,
    params {}
    state { v: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.v = if w == 0.0 { 1.0 / 1e-6 } else { 1.0 / w };
    }
    calc |s, st, _rng, _g| {
        let t = st.tx * st.tx + st.ty * st.ty;
        let r = 1.0 / (t.sqrt() * (t + s.v));
        st.px += st.tx * r;
        st.py += st.ty * r;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `separation`.
    ///
    /// Uses a strict `>`, so zero takes the else branch. In that branch the
    /// whole term is subtracted while the inside-term is added.
    Separation, "separation", Pass::Normal,
    params {
        "separation_x" => x = 1.0, reset = 1.0,
        "separation_y" => y = 1.0, reset = 1.0,
        "separation_xinside" => xinside = 0.0,
        "separation_yinside" => yinside = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        if st.tx > 0.0 {
            st.px += s.w * ((st.tx * st.tx + s.x * s.x).sqrt() - st.tx * s.xinside);
        } else {
            st.px -= s.w * ((st.tx * st.tx + s.x * s.x).sqrt() + st.tx * s.xinside);
        }
        if st.ty > 0.0 {
            st.py += s.w * ((st.ty * st.ty + s.y * s.y).sqrt() - st.ty * s.yinside);
        } else {
            st.py -= s.w * ((st.ty * st.ty + s.y * s.y).sqrt() + st.ty * s.yinside);
        }
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `splits`. Uses `>=`, unlike `separation`'s `>`.
    Splits, "splits", Pass::Normal,
    params {
        "splits_x" => x = 0.0,
        "splits_y" => y = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        st.px += if st.tx >= 0.0 { s.w * (st.tx + s.x) } else { s.w * (st.tx - s.x) };
        st.py += if st.ty >= 0.0 { s.w * (st.ty + s.y) } else { s.w * (st.ty - s.y) };
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `waves2`. Cross-coupled: x uses ty*freqx, y uses tx*freqy.
    Waves2, "waves2", Pass::Normal,
    params {
        "waves2_freqx" => freqx = 2.0, reset = 2.0,
        "waves2_freqy" => freqy = 2.0, reset = 2.0,
        "waves2_freqz" => freqz = 0.0,
        "waves2_scalex" => scalex = 1.0, reset = 1.0,
        "waves2_scaley" => scaley = 1.0, reset = 1.0,
        "waves2_scalez" => scalez = 0.0,
    }
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        st.px += s.w * (st.tx + s.scalex * (st.ty * s.freqx).sin());
        st.py += s.w * (st.ty + s.scaley * (st.tx * s.freqy).sin());
        st.pz += s.w
            * (st.tz
                + s.scalez * ((st.tx * st.tx + st.ty * st.ty).sqrt() * s.freqz).sin());
    }
}

variation! {
    /// `wedge`.
    Wedge, "wedge", Pass::Normal,
    params {
        "wedge_angle" => angle = core::f64::consts::FRAC_PI_2,
            reset = core::f64::consts::FRAC_PI_2,
        "wedge_hole" => hole = 0.0,
        // Clamped to >= 1 then rounded (banker's), written back.
        "wedge_count" => count = 2.0,
            coerce = |v: f64| (if v < 1.0 { 1.0 } else { v }).round_ties_even(),
            reset = 2.0,
        "wedge_swirl" => swirl = 0.0,
    }
    state { comp_fac: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        const C1_2PI: f64 = 0.159_154_943_091_895_335_768_883_763_372_51;
        s.comp_fac = 1.0 - s.angle * s.count * C1_2PI;
    }
    calc |s, st, _rng, _g| {
        const C1_2PI: f64 = 0.159_154_943_091_895_335_768_883_763_372_51;
        let r = (st.tx * st.tx + st.ty * st.ty).sqrt();
        let a = st.ty.atan2(st.tx) + s.swirl * r;
        // True floor here, unlike rings2's truncation.
        let c = ((s.count * a + core::f64::consts::PI) * C1_2PI).floor();
        let a = a * s.comp_fac + c * s.angle;
        let (sina, cosa) = a.sin_cos();

        let r = s.w * (r + s.hole);
        st.px += r * cosa;
        st.py += r * sina;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `pre_disc` — overwrites FT.
    ///
    /// `arctan2` has its arguments swapped AND the sin/cos assignment is
    /// swapped relative to the usual convention (sin -> x, cos -> y).
    PreDisc, "pre_disc", Pass::Pre,
    params {}
    state { vvar_by_pi: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.vvar_by_pi = w / core::f64::consts::PI;
    }
    calc |s, st, _rng, _g| {
        let (sinr, cosr) =
            (core::f64::consts::PI * (st.tx * st.tx + st.ty * st.ty).sqrt()).sin_cos();
        // Compute r before storing — both outputs depend on the old tx/ty.
        let r = s.vvar_by_pi * st.tx.atan2(st.ty);
        st.tx = sinr * r;
        st.ty = cosr * r;
        st.tz = s.w * st.tz;
    }
}

variation! {
    /// `pre_sinusoidal` — overwrites FT.
    PreSinusoidal, "pre_sinusoidal", Pass::Pre,
    params {}
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        st.tx = s.w * st.tx.sin();
        st.ty = s.w * st.ty.sin();
        st.tz = s.w * st.tz;
    }
}

variation! {
    /// `pre_spherical` — overwrites FT.
    ///
    /// The guard is `10e-6` in Pascal, which is 1e-5, NOT 1e-6. Getting this
    /// wrong is a classic porting bug.
    PreSpherical, "pre_spherical", Pass::Pre,
    params {}
    state {}
    prepare |_s, _w, _c, _rng| {}
    calc |s, st, _rng, _g| {
        let r = s.w / (st.tx * st.tx + st.ty * st.ty + 1.0e-5);
        st.tx *= r;
        st.ty *= r;
        st.tz = s.w * st.tz;
    }
}

pub const NAMES: [&str; 9] = [
    "rings2",
    "scry",
    "separation",
    "splits",
    "waves2",
    "wedge",
    "pre_disc",
    "pre_sinusoidal",
    "pre_spherical",
];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "rings2" => Some(Box::new(Rings2::default())),
        "scry" => Some(Box::new(Scry::default())),
        "separation" => Some(Box::new(Separation::default())),
        "splits" => Some(Box::new(Splits::default())),
        "waves2" => Some(Box::new(Waves2::default())),
        "wedge" => Some(Box::new(Wedge::default())),
        "pre_disc" => Some(Box::new(PreDisc::default())),
        "pre_sinusoidal" => Some(Box::new(PreSinusoidal::default())),
        "pre_spherical" => Some(Box::new(PreSpherical::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::{VarState, Variation};

    /// pre_spherical's guard is 1e-5, not 1e-6. At the origin the output is
    /// therefore w/1e-5 = 1e5 times the coordinate, i.e. exactly 0 — but the
    /// constant shows up as soon as the point is near the origin.
    #[test]
    fn pre_spherical_guard_is_1e5() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = PreSpherical::default();
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { tx: 0.0, ty: 0.0, tz: 1.0, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert_eq!(st.tx, 0.0);

        // A point at radius^2 = 1e-5 should halve, since r = w/(1e-5 + 1e-5).
        let mut st = VarState { tx: (1e-5f64).sqrt(), ty: 0.0, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        let expected = (1e-5f64).sqrt() * (1.0 / 2e-5);
        assert!(
            (st.tx - expected).abs() / expected < 1e-9,
            "guard constant is wrong: got {} want {}",
            st.tx,
            expected
        );
    }

    /// pre_ variations overwrite FT and must leave FP alone.
    #[test]
    fn pre_variations_do_not_touch_the_accumulator() {
        let mut rng = Rng::new(2);
        let mut g = GaussBuf::new(&mut rng);

        for v in [
            Box::new(PreDisc::default()) as Box<dyn Variation>,
            Box::new(PreSinusoidal::default()),
            Box::new(PreSpherical::default()),
        ] {
            let mut v = v;
            v.prepare(1.0, &Affine::IDENTITY, &mut rng);
            let mut st = VarState { tx: 0.3, ty: 0.5, tz: 1.0, ..Default::default() };
            v.calc(&mut st, &mut rng, &mut g);
            assert_eq!(st.px, 0.0, "{} wrote FP", v.name());
            assert_eq!(st.py, 0.0, "{} wrote FP", v.name());
        }
    }

    /// separation uses `>`, splits uses `>=`. At exactly zero they diverge.
    #[test]
    fn separation_and_splits_differ_at_zero() {
        let mut rng = Rng::new(3);
        let mut g = GaussBuf::new(&mut rng);

        let mut sp = Splits::default();
        sp.x = 1.0;
        sp.prepare(1.0, &Affine::IDENTITY, &mut rng);
        let mut st = VarState { tx: 0.0, ty: 0.0, ..Default::default() };
        sp.calc(&mut st, &mut rng, &mut g);
        // tx == 0 takes the `>=` branch, so +x.
        assert_eq!(st.px, 1.0, "splits at zero should add splits_x");
    }

    #[test]
    fn wedge_count_is_clamped_and_rounded() {
        let mut v = Wedge::default();
        assert_eq!(v.set_param("wedge_count", 0.2), Some(1.0));
        assert_eq!(v.set_param("wedge_count", -7.0), Some(1.0));
        assert_eq!(v.set_param("wedge_count", 3.6), Some(4.0));
    }
}
