// Ported from varJuliaN.pas, varJuliaScope.pas, varJulia3Djf.pas, varJulia3Dz.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// `julia3D` and `julia3Dz` are easy to conflate and must NOT share code. They
// differ in three places:
//   * cN is (1/N - 1)/2 for julia3D, but 1/N/2 for julia3Dz
//   * CalcPowerMinus2's y sign is minus for julia3D, plus for julia3Dz
//   * CalcPowerMinus1's radius is 3D for julia3D, 2D for julia3Dz
//
// The `c = 1` gate in julian/juliascope is an EXACT float compare in the
// original, so `julian_dist="1.0000001"` silently takes the slow generic path
// and produces different output at power +-2. Preserved deliberately.
//
// Note these use the conventional `arctan2(y, x)` argument order, unlike the
// built-in polar family which swaps them.

use crate::variation::Pass;

/// `*_power` rounds to an integer and forbids zero, writing the coerced value
/// back — the original signals this through its `var` parameter.
fn coerce_power(v: f64) -> f64 {
    let n = v.round_ties_even();
    if n == 0.0 {
        1.0
    } else {
        n
    }
}

/// Reset toggles the sign rather than zeroing: 2 becomes -2, anything else 2.
fn toggle_power(cur: f64) -> f64 {
    if cur == 2.0 {
        -2.0
    } else {
        2.0
    }
}

variation! {
    /// `julian`.
    Julian, "julian", Pass::Normal,
    params {
        "julian_power" => power = 2.0, coerce = coerce_power, reset_fn = toggle_power,
        "julian_dist" => dist = 1.0, reset = 1.0,
    }
    state { abs_n: f64 = 0.0, cn: f64 = 0.0, vvar2: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.abs_n = s.power.abs();
        s.cn = s.dist / s.power / 2.0;
        s.vvar2 = w * core::f64::consts::SQRT_2 / 2.0;
    }
    calc |s, st, rng, _g| {
        let (x, y) = (st.tx, st.ty);

        // Exact compare, as in the original.
        if s.dist == 1.0 {
            if s.power == 2.0 {
                let d = ((x * x + y * y).sqrt() + x).sqrt();
                if rng.below(2) == 0 {
                    st.px += s.vvar2 * d;
                    st.py += s.vvar2 / d * y;
                } else {
                    st.px -= s.vvar2 * d;
                    st.py -= s.vvar2 / d * y;
                }
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == -2.0 {
                let r0 = (x * x + y * y).sqrt();
                let xd = r0 + x;
                let r = s.w / (r0 * (y * y + xd * xd)).sqrt();
                if rng.below(2) == 0 {
                    st.px += r * xd;
                    st.py -= r * y;
                } else {
                    st.px -= r * xd;
                    st.py += r * y;
                }
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == 1.0 {
                st.px += s.w * x;
                st.py += s.w * y;
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == -1.0 {
                let r = s.w / (x * x + y * y);
                st.px += r * x;
                st.py -= r * y;
                st.pz += s.w * st.tz;
                return;
            }
        }

        let a = (y.atan2(x) + core::f64::consts::TAU * rng.below(s.abs_n as u32) as f64) / s.power;
        let (sina, cosa) = a.sin_cos();
        let r = s.w * (x * x + y * y).powf(s.cn);
        st.px += r * cosa;
        st.py += r * sina;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `juliascope` — like julian, but alternate branches mirror the angle.
    Juliascope, "juliascope", Pass::Normal,
    params {
        "juliascope_power" => power = 2.0, coerce = coerce_power, reset_fn = toggle_power,
        "juliascope_dist" => dist = 1.0, reset = 1.0,
    }
    state { abs_n: f64 = 0.0, cn: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.abs_n = s.power.abs();
        s.cn = s.dist / s.power / 2.0;
    }
    calc |s, st, rng, _g| {
        let (x, y) = (st.tx, st.ty);

        if s.dist == 1.0 {
            if s.power == 2.0 {
                let a = if rng.below(2) == 0 {
                    y.atan2(x) / 2.0
                } else {
                    core::f64::consts::PI - y.atan2(x) / 2.0
                };
                let (sina, cosa) = a.sin_cos();
                let r = s.w * (x * x + y * y).sqrt().sqrt();
                st.px += r * cosa;
                st.py += r * sina;
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == -2.0 {
                let a = if rng.below(2) == 0 {
                    y.atan2(x) / 2.0
                } else {
                    core::f64::consts::PI - y.atan2(x) / 2.0
                };
                let (sina, cosa) = a.sin_cos();
                let r = s.w / (x * x + y * y).sqrt().sqrt();
                st.px += r * cosa;
                st.py -= r * sina;
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == 1.0 {
                st.px += s.w * x;
                st.py += s.w * y;
                st.pz += s.w * st.tz;
                return;
            }
            if s.power == -1.0 {
                let r = s.w / (x * x + y * y);
                st.px += r * x;
                st.py -= r * y;
                st.pz += s.w * st.tz;
                return;
            }
        }

        let rnd = rng.below(s.abs_n as u32);
        let theta = y.atan2(x);
        let a = if rnd & 1 == 0 {
            (core::f64::consts::TAU * rnd as f64 + theta) / s.power
        } else {
            (core::f64::consts::TAU * rnd as f64 - theta) / s.power
        };
        let (sina, cosa) = a.sin_cos();
        let r = s.w * (x * x + y * y).powf(s.cn);
        st.px += r * cosa;
        st.py += r * sina;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `julia3D` (varJulia3Djf.pas). cN = (1/N - 1)/2.
    Julia3D, "julia3D", Pass::Normal,
    params {
        "julia3D_power" => power = 2.0, coerce = coerce_power, reset_fn = toggle_power,
    }
    state { abs_n: f64 = 0.0, cn: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.abs_n = s.power.abs();
        s.cn = (1.0 / s.power - 1.0) / 2.0;
    }
    calc |s, st, rng, _g| {
        let (x, y) = (st.tx, st.ty);

        if s.power == 2.0 {
            let z = st.tz / 2.0;
            let r2d = x * x + y * y;
            let r = s.w / (r2d + z * z).sqrt().sqrt();
            st.pz += r * z;
            let tmp = r * r2d.sqrt();
            let a = y.atan2(x) / 2.0 + core::f64::consts::PI * rng.below(2) as f64;
            let (sina, cosa) = a.sin_cos();
            st.px += tmp * cosa;
            st.py += tmp * sina;
            return;
        }
        if s.power == -2.0 {
            let z = st.tz / 2.0;
            let r2d = x * x + y * y;
            let r3d = (r2d + z * z).sqrt();
            let r = s.w / (r3d.sqrt() * r3d);
            st.pz += r * z;
            let tmp = r * r2d.sqrt();
            let a = y.atan2(x) / 2.0 + core::f64::consts::PI * rng.below(2) as f64;
            let (sina, cosa) = a.sin_cos();
            st.px += tmp * cosa;
            st.py -= tmp * sina;
            return;
        }
        if s.power == 1.0 {
            st.px += s.w * x;
            st.py += s.w * y;
            st.pz += s.w * st.tz;
            return;
        }
        if s.power == -1.0 {
            // 3D radius here — julia3Dz uses the 2D one.
            let r = s.w / (x * x + y * y + st.tz * st.tz);
            st.px += r * x;
            st.py -= r * y;
            st.pz += r * st.tz;
            return;
        }

        let z = st.tz / s.abs_n;
        let r2d = x * x + y * y;
        let r = s.w * (r2d + z * z).powf(s.cn);
        st.pz += r * z;
        let tmp = r * r2d.sqrt();
        let a = (y.atan2(x) + core::f64::consts::TAU * rng.below(s.abs_n as u32) as f64) / s.power;
        let (sina, cosa) = a.sin_cos();
        st.px += tmp * cosa;
        st.py += tmp * sina;
    }
}

variation! {
    /// `julia3Dz`. cN = 1/N/2, and its z term divides by the 2D radius.
    Julia3Dz, "julia3Dz", Pass::Normal,
    params {
        "julia3Dz_power" => power = 2.0, coerce = coerce_power, reset_fn = toggle_power,
    }
    state { abs_n: f64 = 0.0, cn: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.abs_n = s.power.abs();
        s.cn = 1.0 / s.power / 2.0;
    }
    calc |s, st, rng, _g| {
        let (x, y) = (st.tx, st.ty);

        if s.power == 2.0 {
            // Note r2d is a RADIUS in these two kernels, not a squared radius.
            let r2d = (x * x + y * y).sqrt();
            let r = s.w * r2d.sqrt();
            st.pz += r * st.tz / r2d / 2.0;
            let a = y.atan2(x) / 2.0 + core::f64::consts::PI * rng.below(2) as f64;
            let (sina, cosa) = a.sin_cos();
            st.px += r * cosa;
            st.py += r * sina;
            return;
        }
        if s.power == -2.0 {
            let r2d = (x * x + y * y).sqrt();
            let r = s.w / r2d.sqrt();
            st.pz += r * st.tz / r2d / 2.0;
            let a = core::f64::consts::PI * rng.below(2) as f64 - y.atan2(x) / 2.0;
            let (sina, cosa) = a.sin_cos();
            st.px += r * cosa;
            // Plus here — julia3D uses minus in the corresponding kernel.
            st.py += r * sina;
            return;
        }
        if s.power == 1.0 {
            st.px += s.w * x;
            st.py += s.w * y;
            st.pz += s.w * st.tz;
            return;
        }
        if s.power == -1.0 {
            // 2D radius here — julia3D uses the 3D one.
            let r = s.w / (x * x + y * y);
            st.px += r * x;
            st.py -= r * y;
            st.pz += r * st.tz;
            return;
        }

        let r2d = x * x + y * y;
        let r = s.w * r2d.powf(s.cn);
        st.pz += r * st.tz / (r2d.sqrt() * s.abs_n);
        let a = (y.atan2(x) + core::f64::consts::TAU * rng.below(s.abs_n as u32) as f64) / s.power;
        let (sina, cosa) = a.sin_cos();
        st.px += r * cosa;
        st.py += r * sina;
    }
}

pub const NAMES: [&str; 4] = ["julian", "juliascope", "julia3D", "julia3Dz"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "julian" => Some(Box::new(Julian::default())),
        "juliascope" => Some(Box::new(Juliascope::default())),
        "julia3D" => Some(Box::new(Julia3D::default())),
        "julia3Dz" => Some(Box::new(Julia3Dz::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::{VarState, Variation};

    #[test]
    fn power_is_rounded_and_never_zero() {
        let mut v = Julian::default();
        assert_eq!(v.set_param("julian_power", 3.4), Some(3.0));
        assert_eq!(v.set_param("julian_power", 0.0), Some(1.0), "zero must be forbidden");
        assert_eq!(v.set_param("julian_power", -2.6), Some(-3.0));
    }

    #[test]
    fn power_reset_toggles_sign() {
        let mut v = Julian::default();
        v.set_param("julian_power", 2.0);
        assert_eq!(v.reset_param("julian_power"), Some(-2.0));
        assert_eq!(v.reset_param("julian_power"), Some(2.0));
    }

    /// julia3D and julia3Dz must not be conflated — different cN and z terms.
    #[test]
    fn julia3d_variants_differ() {
        let mut rng = Rng::new(7);
        let mut g = GaussBuf::new(&mut rng);

        let run = |v: &mut dyn Variation, rng: &mut Rng, g: &mut GaussBuf| {
            v.prepare(1.0, &Affine::IDENTITY, rng);
            let mut st = VarState { tx: 0.5, ty: 0.3, tz: 0.7, ..Default::default() };
            v.calc(&mut st, rng, g);
            (st.px, st.py, st.pz)
        };

        let mut a = Julia3D::default();
        a.power = 5.0;
        let mut b = Julia3Dz::default();
        b.power = 5.0;

        let ra = run(&mut a, &mut Rng::new(1), &mut g);
        let rb = run(&mut b, &mut Rng::new(1), &mut g);
        assert!(
            (ra.2 - rb.2).abs() > 1e-9,
            "z outputs should differ: {} vs {}",
            ra.2,
            rb.2
        );
    }

    /// The dist gate is an exact compare, so 1.0000001 takes the generic path.
    #[test]
    fn dist_gate_is_exact() {
        let mut rng = Rng::new(3);
        let mut g = GaussBuf::new(&mut rng);

        let mut exact = Julian::default();
        exact.power = 2.0;
        exact.dist = 1.0;
        exact.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut near = Julian::default();
        near.power = 2.0;
        near.dist = 1.0000001;
        near.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut s1 = VarState { tx: 0.5, ty: 0.25, ..Default::default() };
        let mut s2 = s1;
        exact.calc(&mut s1, &mut Rng::new(9), &mut g);
        near.calc(&mut s2, &mut Rng::new(9), &mut g);
        assert!(
            (s1.px - s2.px).abs() > 1e-9,
            "a hair off 1.0 must take a different code path"
        );
    }
}
