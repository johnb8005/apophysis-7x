// Ported from varFalloff2.pas, varPreFalloff2.pas, varPostFalloff2.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// The three are identical except for where they read from and write to. All
// three READ AND WRITE the colour coordinate, which makes them the first
// direct-colour variations in the port.
//
// A Pascal layout trap worth spelling out. The source writes:
//
//     if (invert <> 0) then d := 1 - d; if (d < 0) then d := 0;
//
// That is TWO statements on one line — the `d < 0` clamp is NOT part of the
// `if invert` branch and runs unconditionally. Same for the line after it.

use crate::rng::Rng;
use crate::variation::{Pass, VarState};

/// Falloff distance ramp, shared by all three variants and all three modes.
#[inline]
fn falloff_d(
    x: f64,
    y: f64,
    z: f64,
    x0: f64,
    y0: f64,
    z0: f64,
    invert: bool,
    mindist: f64,
    rmax: f64,
) -> f64 {
    let mut d = ((x - x0).powi(2) + (y - y0).powi(2) + (z - z0).powi(2)).sqrt();
    if invert {
        d = 1.0 - d;
    }
    // Unconditional — see the module comment.
    if d < 0.0 {
        d = 0.0;
    }
    d = (d - mindist) * rmax;
    if d < 0.0 {
        d = 0.0;
    }
    d
}

/// The three blur modes. `blurtype` is clamped to [0, 2] on set.
#[derive(Clone, Copy)]
enum Mode {
    Uniform,
    Radial,
    Gaussian,
}

fn mode_of(t: f64) -> Mode {
    match t as i32 {
        1 => Mode::Radial,
        2 => Mode::Gaussian,
        _ => Mode::Uniform,
    }
}

/// Compute the displaced point for a mode. Returns `(x, y, z)`.
///
/// RNG draw order is load-bearing and differs per mode; all three then draw
/// once more for colour at the call site.
#[inline]
#[allow(clippy::too_many_arguments)]
fn displace(
    mode: Mode,
    in_x: f64,
    in_y: f64,
    in_z: f64,
    d: f64,
    mul_x: f64,
    mul_y: f64,
    mul_z: f64,
    rng: &mut Rng,
) -> (f64, f64, f64) {
    match mode {
        Mode::Uniform => (
            in_x + mul_x * rng.f64() * d,
            in_y + mul_y * rng.f64() * d,
            in_z + mul_z * rng.f64() * d,
        ),
        Mode::Radial => {
            let r_in = (in_x * in_x + in_y * in_y + in_z * in_z).sqrt() + 1e-6;
            let sigma = (in_z / r_in).asin() + mul_z * rng.f64() * d;
            let phi = in_y.atan2(in_x) + mul_y * rng.f64() * d;
            let r = r_in + mul_x * rng.f64() * d;

            let (sins, coss) = sigma.sin_cos();
            let (sinp, cosp) = phi.sin_cos();
            // The z output is `sins` with NO r factor — asymmetric with x/y,
            // and literal in the original.
            (r * coss * cosp, r * coss * sinp, sins)
        }
        Mode::Gaussian => {
            let sigma = d * rng.f64() * 2.0 * core::f64::consts::PI;
            let phi = d * rng.f64() * core::f64::consts::PI;
            let r = d * rng.f64();

            let (sins, coss) = sigma.sin_cos();
            let (sinp, cosp) = phi.sin_cos();
            (
                in_x + mul_x * r * coss * cosp,
                in_y + mul_y * r * coss * sinp,
                // Uses sin(sigma), the same angle as coss above — literal.
                in_z + mul_z * r * sins,
            )
        }
    }
}

/// `color^ := Abs(Frac(color^ + mul_c * random * d))`.
///
/// Delphi `Frac` truncates toward zero, so it keeps the sign of its argument;
/// `Abs` then folds it. Rust's `fract()` has the same truncating semantics.
#[inline]
fn blend_color(vc: f64, mul_c: f64, d: f64, rng: &mut Rng) -> f64 {
    (vc + mul_c * rng.f64() * d).fract().abs()
}

fn clamp_unit(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn clamp_scatter(v: f64) -> f64 {
    if v < 1e-6 {
        1e-6
    } else {
        v
    }
}

fn clamp_mindist(v: f64) -> f64 {
    if v < 0.0 {
        0.0
    } else {
        v
    }
}

fn clamp_invert(v: f64) -> f64 {
    v.clamp(0.0, 1.0).round_ties_even()
}

fn clamp_type(v: f64) -> f64 {
    v.clamp(0.0, 2.0).round_ties_even()
}

/// Shared body: read from `(x, y, z)`, produce the displaced point and colour.
#[inline]
#[allow(clippy::too_many_arguments)]
fn run(
    st: &mut VarState,
    rng: &mut Rng,
    x: f64,
    y: f64,
    z: f64,
    x0: f64,
    y0: f64,
    z0: f64,
    invert: bool,
    mindist: f64,
    rmax: f64,
    mode: Mode,
    mul_x: f64,
    mul_y: f64,
    mul_z: f64,
    mul_c: f64,
) -> (f64, f64, f64) {
    let d = falloff_d(x, y, z, x0, y0, z0, invert, mindist, rmax);
    let out = displace(mode, x, y, z, d, mul_x, mul_y, mul_z, rng);
    st.vc = blend_color(st.vc, mul_c, d, rng);
    out
}


variation! {
    /// `falloff2` — accumulates into FP.
    Falloff2, "falloff2", Pass::Normal,
    params {
        "falloff2_scatter" => scatter = 1.0, coerce = clamp_scatter, reset = 1.0,
        "falloff2_mindist" => mindist = 0.5, coerce = clamp_mindist, reset = 0.5,
        "falloff2_mul_x" => mul_x = 1.0, coerce = clamp_unit, reset = 1.0,
        "falloff2_mul_y" => mul_y = 1.0, coerce = clamp_unit, reset = 1.0,
        "falloff2_mul_z" => mul_z = 0.0, coerce = clamp_unit,
        "falloff2_mul_c" => mul_c = 0.0, coerce = clamp_unit,
        "falloff2_x0" => x0 = 0.0,
        "falloff2_y0" => y0 = 0.0,
        "falloff2_z0" => z0 = 0.0,
        "falloff2_invert" => invert = 0.0, coerce = clamp_invert,
        "falloff2_type" => blurtype = 0.0, coerce = clamp_type,
    }
    state { rmax: f64 = 0.0 }
    prepare |s, _w, _c, _rng| { s.rmax = 0.04 * s.scatter; }
    calc |s, st, rng, _g| {
        let (x, y, z) = (st.tx, st.ty, st.tz);
        let (ox, oy, oz) = run(
            st, rng, x, y, z, s.x0, s.y0, s.z0, s.invert != 0.0, s.mindist, s.rmax,
            mode_of(s.blurtype), s.mul_x, s.mul_y, s.mul_z, s.mul_c,
        );
        st.px += s.w * ox;
        st.py += s.w * oy;
        st.pz += s.w * oz;
    }
}

variation! {
    /// `pre_falloff2` — reads FT and overwrites FT. The weight scales the
    /// coordinate in place, so a weight != 1 rescales the input handed to
    /// every later variation in the xform.
    PreFalloff2, "pre_falloff2", Pass::Pre,
    params {
        "pre_falloff2_scatter" => scatter = 1.0, coerce = clamp_scatter, reset = 1.0,
        "pre_falloff2_mindist" => mindist = 0.5, coerce = clamp_mindist, reset = 0.5,
        "pre_falloff2_mul_x" => mul_x = 1.0, coerce = clamp_unit, reset = 1.0,
        "pre_falloff2_mul_y" => mul_y = 1.0, coerce = clamp_unit, reset = 1.0,
        "pre_falloff2_mul_z" => mul_z = 0.0, coerce = clamp_unit,
        "pre_falloff2_mul_c" => mul_c = 0.0, coerce = clamp_unit,
        "pre_falloff2_x0" => x0 = 0.0,
        "pre_falloff2_y0" => y0 = 0.0,
        "pre_falloff2_z0" => z0 = 0.0,
        "pre_falloff2_invert" => invert = 0.0, coerce = clamp_invert,
        "pre_falloff2_type" => blurtype = 0.0, coerce = clamp_type,
    }
    state { rmax: f64 = 0.0 }
    prepare |s, _w, _c, _rng| { s.rmax = 0.04 * s.scatter; }
    calc |s, st, rng, _g| {
        let (x, y, z) = (st.tx, st.ty, st.tz);
        let (ox, oy, oz) = run(
            st, rng, x, y, z, s.x0, s.y0, s.z0, s.invert != 0.0, s.mindist, s.rmax,
            mode_of(s.blurtype), s.mul_x, s.mul_y, s.mul_z, s.mul_c,
        );
        st.tx = s.w * ox;
        st.ty = s.w * oy;
        st.tz = s.w * oz;
    }
}

variation! {
    /// `post_falloff2` — reads FP and overwrites FP.
    PostFalloff2, "post_falloff2", Pass::Post,
    params {
        "post_falloff2_scatter" => scatter = 1.0, coerce = clamp_scatter, reset = 1.0,
        "post_falloff2_mindist" => mindist = 0.5, coerce = clamp_mindist, reset = 0.5,
        "post_falloff2_mul_x" => mul_x = 1.0, coerce = clamp_unit, reset = 1.0,
        "post_falloff2_mul_y" => mul_y = 1.0, coerce = clamp_unit, reset = 1.0,
        "post_falloff2_mul_z" => mul_z = 0.0, coerce = clamp_unit,
        "post_falloff2_mul_c" => mul_c = 0.0, coerce = clamp_unit,
        "post_falloff2_x0" => x0 = 0.0,
        "post_falloff2_y0" => y0 = 0.0,
        "post_falloff2_z0" => z0 = 0.0,
        "post_falloff2_invert" => invert = 0.0, coerce = clamp_invert,
        "post_falloff2_type" => blurtype = 0.0, coerce = clamp_type,
    }
    state { rmax: f64 = 0.0 }
    prepare |s, _w, _c, _rng| { s.rmax = 0.04 * s.scatter; }
    calc |s, st, rng, _g| {
        let (x, y, z) = (st.px, st.py, st.pz);
        let (ox, oy, oz) = run(
            st, rng, x, y, z, s.x0, s.y0, s.z0, s.invert != 0.0, s.mindist, s.rmax,
            mode_of(s.blurtype), s.mul_x, s.mul_y, s.mul_z, s.mul_c,
        );
        st.px = s.w * ox;
        st.py = s.w * oy;
        st.pz = s.w * oz;
    }
}

pub const NAMES: [&str; 3] = ["falloff2", "pre_falloff2", "post_falloff2"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "falloff2" => Some(Box::new(Falloff2::default())),
        "pre_falloff2" => Some(Box::new(PreFalloff2::default())),
        "post_falloff2" => Some(Box::new(PostFalloff2::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::Variation;

    /// The `d < 0` clamp runs unconditionally, not only when invert is set.
    #[test]
    fn distance_clamp_is_unconditional() {
        // Without invert, a point 5 units out gives d = (5 - mindist)*rmax > 0.
        let d = falloff_d(5.0, 0.0, 0.0, 0.0, 0.0, 0.0, false, 0.5, 0.04);
        assert!(d > 0.0);
        // With invert, 1 - 5 = -4, clamped to 0, then (0 - 0.5)*0.04 < 0,
        // clamped to 0 again.
        let d = falloff_d(5.0, 0.0, 0.0, 0.0, 0.0, 0.0, true, 0.5, 0.04);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn writes_colour_coordinate() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = Falloff2::default();
        v.mul_c = 1.0;
        v.mindist = 0.0;
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { tx: 3.0, ty: 3.0, vc: 0.25, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert_ne!(st.vc, 0.25, "colour should have been blended");
        assert!((0.0..1.0).contains(&st.vc), "colour out of range: {}", st.vc);
    }

    #[test]
    fn scatter_and_type_are_coerced() {
        let mut v = Falloff2::default();
        assert_eq!(v.set_param("falloff2_scatter", -5.0), Some(1e-6));
        assert_eq!(v.set_param("falloff2_type", 9.0), Some(2.0));
        assert_eq!(v.set_param("falloff2_mul_x", 3.0), Some(1.0));
        assert_eq!(v.set_param("falloff2_mindist", -1.0), Some(0.0));
    }
}
