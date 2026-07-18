// Ported from varCrop.pas, varPreCrop.pas, varPostCrop.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// The three are character-identical apart from their output block, but note
// the z handling differs: base `crop` propagates `FPz += vvar * FTz`, while
// `pre_crop` and `post_crop` never touch z at all — not even to scale it.
//
// `crop_scatter_area` and `crop_zero` are absent from the Pascal constructor
// and rely on Delphi zero-initialising object memory, so their defaults are 0
// with no line of source saying so.

use crate::rng::Rng;
use crate::variation::Pass;

/// Sorted bounds plus scatter half-extents (`Prepare`, identical in all three).
#[inline]
fn prep(x0: f64, y0: f64, x1: f64, y1: f64, s: f64) -> (f64, f64, f64, f64, f64, f64) {
    let (lo_x, hi_x) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
    let (lo_y, hi_y) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
    // These go negative when s is negative, which the [-1,1] clamp permits.
    ((lo_x), (lo_y), (hi_x), (hi_y), (hi_x - lo_x) * 0.5 * s, (hi_y - lo_y) * 0.5 * s)
}

/// Clamp a point into the crop rect, scattering it inward by a random amount.
///
/// The RNG is consumed only on the branches that actually scatter, so the
/// number of draws per call is input-dependent. That is load-bearing for
/// stream alignment — do not hoist the draws out of the branches.
#[inline]
#[allow(clippy::too_many_arguments)]
fn crop_point(
    x: f64,
    y: f64,
    lo_x: f64,
    lo_y: f64,
    hi_x: f64,
    hi_y: f64,
    sw: f64,
    sh: f64,
    zero: bool,
    rng: &mut Rng,
) -> (f64, f64) {
    if (x < lo_x || x > hi_x || y < lo_y || y > hi_y) && zero {
        return (0.0, 0.0);
    }

    // Note the sign asymmetry: the low side adds, the high side subtracts.
    let x = if x < lo_x {
        lo_x + rng.f64() * sw
    } else if x > hi_x {
        hi_x - rng.f64() * sw
    } else {
        x
    };
    let y = if y < lo_y {
        lo_y + rng.f64() * sh
    } else if y > hi_y {
        hi_y - rng.f64() * sh
    } else {
        y
    };
    (x, y)
}

/// `*_scatter_area` clamps to [-1, 1].
fn clamp_scatter(v: f64) -> f64 {
    v.clamp(-1.0, 1.0)
}

/// `*_zero` clamps to [0, 1] then rounds. Delphi's `Round` is banker's
/// rounding; after the clamp the only ambiguous input is 0.5, which goes to 0,
/// and `round_ties_even` reproduces that.
fn clamp_zero(v: f64) -> f64 {
    v.clamp(0.0, 1.0).round_ties_even()
}

variation! {
    /// `crop` — accumulates, and is the only one of the three that touches z.
    Crop, "crop", Pass::Normal,
    params {
        "crop_left" => x0 = -1.0, reset = -1.0,
        "crop_top" => y0 = -1.0, reset = -1.0,
        "crop_right" => x1 = 1.0, reset = 1.0,
        "crop_bottom" => y1 = 1.0, reset = 1.0,
        "crop_scatter_area" => scatter = 0.0, coerce = clamp_scatter,
        "crop_zero" => zero = 0.0, coerce = clamp_zero,
    }
    state { lo_x: f64 = 0.0, lo_y: f64 = 0.0, hi_x: f64 = 0.0, hi_y: f64 = 0.0, sw: f64 = 0.0, sh: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (a, b, c, d, e, f) = prep(s.x0, s.y0, s.x1, s.y1, s.scatter);
        s.lo_x = a; s.lo_y = b; s.hi_x = c; s.hi_y = d; s.sw = e; s.sh = f;
    }
    calc |s, st, rng, _g| {
        let (x, y) = crop_point(
            st.tx, st.ty, s.lo_x, s.lo_y, s.hi_x, s.hi_y, s.sw, s.sh, s.zero != 0.0, rng,
        );
        st.px += s.w * x;
        st.py += s.w * y;
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `pre_crop` — overwrites FT. Never touches z.
    PreCrop, "pre_crop", Pass::Pre,
    params {
        "pre_crop_left" => x0 = -1.0, reset = -1.0,
        "pre_crop_top" => y0 = -1.0, reset = -1.0,
        "pre_crop_right" => x1 = 1.0, reset = 1.0,
        "pre_crop_bottom" => y1 = 1.0, reset = 1.0,
        "pre_crop_scatter_area" => scatter = 0.0, coerce = clamp_scatter,
        "pre_crop_zero" => zero = 0.0, coerce = clamp_zero,
    }
    state { lo_x: f64 = 0.0, lo_y: f64 = 0.0, hi_x: f64 = 0.0, hi_y: f64 = 0.0, sw: f64 = 0.0, sh: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (a, b, c, d, e, f) = prep(s.x0, s.y0, s.x1, s.y1, s.scatter);
        s.lo_x = a; s.lo_y = b; s.hi_x = c; s.hi_y = d; s.sw = e; s.sh = f;
    }
    calc |s, st, rng, _g| {
        let (x, y) = crop_point(
            st.tx, st.ty, s.lo_x, s.lo_y, s.hi_x, s.hi_y, s.sw, s.sh, s.zero != 0.0, rng,
        );
        st.tx = s.w * x;
        st.ty = s.w * y;
    }
}

variation! {
    /// `post_crop` — reads and overwrites FP. Never touches z.
    PostCrop, "post_crop", Pass::Post,
    params {
        "post_crop_left" => x0 = -1.0, reset = -1.0,
        "post_crop_top" => y0 = -1.0, reset = -1.0,
        "post_crop_right" => x1 = 1.0, reset = 1.0,
        "post_crop_bottom" => y1 = 1.0, reset = 1.0,
        "post_crop_scatter_area" => scatter = 0.0, coerce = clamp_scatter,
        "post_crop_zero" => zero = 0.0, coerce = clamp_zero,
    }
    state { lo_x: f64 = 0.0, lo_y: f64 = 0.0, hi_x: f64 = 0.0, hi_y: f64 = 0.0, sw: f64 = 0.0, sh: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (a, b, c, d, e, f) = prep(s.x0, s.y0, s.x1, s.y1, s.scatter);
        s.lo_x = a; s.lo_y = b; s.hi_x = c; s.hi_y = d; s.sw = e; s.sh = f;
    }
    calc |s, st, rng, _g| {
        let (x, y) = crop_point(
            st.px, st.py, s.lo_x, s.lo_y, s.hi_x, s.hi_y, s.sw, s.sh, s.zero != 0.0, rng,
        );
        st.px = s.w * x;
        st.py = s.w * y;
    }
}

pub const NAMES: [&str; 3] = ["crop", "pre_crop", "post_crop"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "crop" => Some(Box::new(Crop::default())),
        "pre_crop" => Some(Box::new(PreCrop::default())),
        "post_crop" => Some(Box::new(PostCrop::default())),
        _ => None,
    }
}
