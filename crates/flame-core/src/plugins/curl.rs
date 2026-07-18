// Ported from varCurl.pas, varCurl3D.pas, varPostCurl.pas, varPostCurl3D.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// These four look like two pairs but are four distinct implementations:
//
//   * `curl` applies the weight to its OUTPUT (`r := vvar / ...`), while
//     `post_curl` folds the weight into its COEFFICIENTS during Prepare and
//     divides by a bare `r`. They are not the same function post-composed.
//   * `curl3D` and `post_curl3D` differ in the sign of the y numerator
//     (`- cy*r2` vs `+ _cy*r2`) despite sharing the denominator's `- c2y*y`.
//   * `curl3D`'s GetCalcFunction ends with an unconditional
//     `f := CalcFunction`, so its four specialised kernels are dead code.
//     Only the general kernel is ported.

use crate::plugins::is_zero;
use crate::variation::Pass;

variation! {
    /// `curl` — f(z) = z / (c2*z^2 + c1*z + 1) over the complex plane.
    Curl, "curl", Pass::Normal,
    params {
        "curl_c1" => c1 = 0.0,
        "curl_c2" => c2 = 0.0,
    }
    state { c2x2: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.c2x2 = 2.0 * s.c2;
    }
    calc |s, st, _rng, _g| {
        let (x, y) = (st.tx, st.ty);

        // The original dispatches to four kernels on IsZero(c1)/IsZero(c2).
        // They compute the same expression with terms dropped, so branching
        // here reproduces them exactly — including that IsZero is an epsilon
        // compare, so c1 = 1e-13 really does take the zero path.
        let c1z = is_zero(s.c1);
        let c2z = is_zero(s.c2);

        let (re, im) = match (c1z, c2z) {
            (true, true) => {
                st.px += s.w * x;
                st.py += s.w * y;
                st.pz += s.w * st.tz;
                return;
            }
            (true, false) => (1.0 + s.c2 * (x * x - y * y), s.c2x2 * x * y),
            (false, true) => (1.0 + s.c1 * x, s.c1 * y),
            (false, false) => {
                (1.0 + s.c1 * x + s.c2 * (x * x - y * y), s.c1 * y + s.c2x2 * x * y)
            }
        };

        let r = s.w / (re * re + im * im);
        st.px += (x * re + y * im) * r;
        st.py += (y * re - x * im) * r;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `curl3D`. Note the y sign asymmetry: `- c2y*y` in the denominator and
    /// `- cy*r2` in the numerator, while x and z use `+`.
    Curl3D, "curl3D", Pass::Normal,
    params {
        "curl3D_cx" => cx = 0.0,
        "curl3D_cy" => cy = 0.0,
        "curl3D_cz" => cz = 0.0,
    }
    state { c2x: f64 = 0.0, c2y: f64 = 0.0, c2z: f64 = 0.0, c2: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        s.c2x = 2.0 * s.cx;
        s.c2y = 2.0 * s.cy;
        s.c2z = 2.0 * s.cz;
        s.c2 = s.cx * s.cx + s.cy * s.cy + s.cz * s.cz;
    }
    calc |s, st, _rng, _g| {
        let (x, y, z) = (st.tx, st.ty, st.tz);
        let r2 = x * x + y * y + z * z;
        let r = s.w / (r2 * s.c2 + s.c2x * x - s.c2y * y + s.c2z * z + 1.0);

        st.px += r * (x + s.cx * r2);
        st.py += r * (y - s.cy * r2);
        st.pz += r * (z + s.cz * r2);
    }
}

variation! {
    /// `post_curl` — reads and overwrites FP, never touches z.
    ///
    /// The weight multiplies the coefficients, not the result. The original
    /// does this by mutating its own parameter fields in `Prepare`, which is
    /// destructive on a second call; we keep the raw params and scale copies.
    PostCurl, "post_curl", Pass::Post,
    params {
        "post_curl_c1" => c1 = 0.0,
        "post_curl_c2" => c2 = 0.0,
    }
    state { s1: f64 = 0.0, s2: f64 = 0.0, c22: f64 = 0.0 }
    prepare |s, w, _c, _rng| {
        s.s1 = s.c1 * w;
        s.s2 = s.c2 * w;
        s.c22 = 2.0 * s.s2;
    }
    calc |s, st, _rng, _g| {
        let (x, y) = (st.px, st.py);

        let re = 1.0 + s.s1 * x + s.s2 * (x * x - y * y);
        let im = s.s1 * y + s.c22 * x * y;

        // No weight factor on the output, and no epsilon on r — with c1=c2=0
        // this is exactly the identity (re=1, im=0, r=1).
        let r = re * re + im * im;
        st.px = (x * re + y * im) / r;
        st.py = (y * re - x * im) / r;
    }
}

variation! {
    /// `post_curl3D` — reads and overwrites FP.
    ///
    /// Differs from `curl3D` by more than its target: the y numerator is
    /// `+ _cy*r2` here versus `- cy*r2` there.
    PostCurl3D, "post_curl3D", Pass::Post,
    params {
        "post_curl3D_cx" => cx = 0.0,
        "post_curl3D_cy" => cy = 0.0,
        "post_curl3D_cz" => cz = 0.0,
    }
    state {
        sx: f64 = 0.0, sy: f64 = 0.0, sz: f64 = 0.0,
        c2x: f64 = 0.0, c2y: f64 = 0.0, c2z: f64 = 0.0, c2: f64 = 0.0
    }
    prepare |s, w, _c, _rng| {
        s.sx = w * s.cx;
        s.sy = w * s.cy;
        s.sz = w * s.cz;
        s.c2x = 2.0 * s.sx;
        s.c2y = 2.0 * s.sy;
        s.c2z = 2.0 * s.sz;
        s.c2 = s.sx * s.sx + s.sy * s.sy + s.sz * s.sz;
    }
    calc |s, st, _rng, _g| {
        // The clamp is in the original, with the comment
        // "// <--- got weird FP overflow there...".
        const LIM: f64 = 1e100;
        let x = st.px.clamp(-LIM, LIM);
        let y = st.py.clamp(-LIM, LIM);
        let z = st.pz.clamp(-LIM, LIM);

        let r2 = x * x + y * y + z * z;
        let r = 1.0 / (r2 * s.c2 + s.c2x * x - s.c2y * y + s.c2z * z + 1.0);

        st.px = r * (x + s.sx * r2);
        st.py = r * (y + s.sy * r2);
        st.pz = r * (z + s.sz * r2);
    }
}

pub const NAMES: [&str; 4] = ["curl", "curl3D", "post_curl", "post_curl3D"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "curl" => Some(Box::new(Curl::default())),
        "curl3D" => Some(Box::new(Curl3D::default())),
        "post_curl" => Some(Box::new(PostCurl::default())),
        "post_curl3D" => Some(Box::new(PostCurl3D::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::Affine;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::{VarState, Variation};

    /// curl3D and post_curl3D must not be conflated: same input, different y.
    #[test]
    fn curl3d_and_post_curl3d_differ_in_y_sign() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);

        let mut a = Curl3D::default();
        a.cy = 0.5;
        a.prepare(1.0, &Affine::IDENTITY, &mut rng);
        let mut sa = VarState { tx: 0.3, ty: 0.4, tz: 0.0, ..Default::default() };
        a.calc(&mut sa, &mut rng, &mut g);

        let mut b = PostCurl3D::default();
        b.cy = 0.5;
        b.prepare(1.0, &Affine::IDENTITY, &mut rng);
        let mut sb = VarState { px: 0.3, py: 0.4, pz: 0.0, ..Default::default() };
        b.calc(&mut sb, &mut rng, &mut g);

        assert!(
            (sa.py - sb.py).abs() > 1e-9,
            "y outputs should differ in sign handling: {} vs {}",
            sa.py,
            sb.py
        );
    }

    /// post_curl with zero coefficients is the identity, not a no-op crash.
    #[test]
    fn post_curl_zero_coefficients_is_identity() {
        let mut rng = Rng::new(2);
        let mut g = GaussBuf::new(&mut rng);
        let mut v = PostCurl::default();
        v.prepare(1.0, &Affine::IDENTITY, &mut rng);

        let mut st = VarState { px: 0.7, py: -0.2, ..Default::default() };
        v.calc(&mut st, &mut rng, &mut g);
        assert!((st.px - 0.7).abs() < 1e-12, "px: {}", st.px);
        assert!((st.py + 0.2).abs() < 1e-12, "py: {}", st.py);
    }

    /// Re-preparing must not compound the weight into the coefficients.
    #[test]
    fn post_curl_prepare_is_not_destructive() {
        let mut rng = Rng::new(3);
        let mut v = PostCurl::default();
        v.c1 = 0.5;
        v.prepare(2.0, &Affine::IDENTITY, &mut rng);
        let first = v.s1;
        v.prepare(2.0, &Affine::IDENTITY, &mut rng);
        assert_eq!(first, v.s1, "weight was folded in twice");
        assert_eq!(v.c1, 0.5, "raw parameter was mutated");
    }
}
