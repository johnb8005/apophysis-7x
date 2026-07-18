// Ported from varBwraps.pas, varPreBwraps.pas, varPostBwraps.pas.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// The three share one kernel verbatim, but they are NOT wrappers around a
// common implementation. Two differences are load-bearing:
//
//   * base `bwraps` has explicit pass-through branches for `cellsize == 0` and
//     for points outside the cell radius, both emitting `FP += vvar * FT`.
//     `pre_` and `post_` invert those conditions and have no else branch, so
//     such points are left COMPLETELY untouched. Factoring the kernel would
//     silently give them the base's fallback and change every render.
//   * the cellsize zero-guard sits in `SetVariable` for `post_bwraps` only.
//     For `bwraps` and `pre_bwraps` it sits in `GetVariable`, where the next
//     line overwrites it — i.e. it is dead code, and cellsize really can be 0
//     at render time. That is exactly why the base needs its `== 0` branch.

use crate::variation::Pass;
#[cfg(test)]
use crate::variation::VarState;

/// Shared precomputation (`Prepare` is identical in all three).
#[inline]
fn prep(cellsize: f64, space: f64, gain: f64) -> (f64, f64, f64) {
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = (gain * gain) / (radius + 1e-6) + 1e-6;

    let mut max_bubble = g2 * radius;
    if max_bubble > 2.0 {
        max_bubble = 1.0;
    } else {
        max_bubble *= 1.0 / (max_bubble * max_bubble / 4.0 + 1.0);
    }

    (g2, radius * radius, radius / max_bubble)
}

/// The cell-warp itself. Returns `None` when the point is outside the cell
/// radius — the caller decides what that means, which is the whole point.
#[inline]
fn warp(
    vx: f64,
    vy: f64,
    cellsize: f64,
    inner_twist: f64,
    outer_twist: f64,
    g2: f64,
    r2: f64,
    rfactor: f64,
) -> Option<(f64, f64)> {
    // Math.Floor in Delphi is a true floor toward -inf, not truncation. Cell
    // indices go negative constantly, so this distinction matters.
    let cx = ((vx / cellsize).floor() + 0.5) * cellsize;
    let cy = ((vy / cellsize).floor() + 0.5) * cellsize;

    let mut lx = vx - cx;
    let mut ly = vy - cy;

    if lx * lx + ly * ly > r2 {
        return None;
    }

    lx *= g2;
    ly *= g2;

    let r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx *= r;
    ly *= r;

    let r = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r) + outer_twist * r;
    let (s, c) = theta.sin_cos();

    Some((cx + c * lx + s * ly, cy - s * lx + c * ly))
}

variation! {
    /// `bwraps` — bubble-wrap cells. Accumulates, with pass-through fallbacks.
    Bwraps, "bwraps", Pass::Normal,
    params {
        "bwraps_cellsize" => cellsize = 1.0, reset = 1.0,
        "bwraps_space" => space = 0.0,
        "bwraps_gain" => gain = 1.0, reset = 1.0,
        "bwraps_inner_twist" => inner_twist = 0.0,
        "bwraps_outer_twist" => outer_twist = 0.0,
    }
    state { g2: f64 = 0.0, r2: f64 = 0.0, rfactor: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (g2, r2, rfactor) = prep(s.cellsize, s.space, s.gain);
        s.g2 = g2;
        s.r2 = r2;
        s.rfactor = rfactor;
    }
    calc |s, st, _rng, _g| {
        // cellsize can legitimately be 0 here — the setter does not guard it.
        let warped = if s.cellsize == 0.0 {
            None
        } else {
            warp(st.tx, st.ty, s.cellsize, s.inner_twist, s.outer_twist, s.g2, s.r2, s.rfactor)
        };

        match warped {
            Some((vx, vy)) => {
                st.px += s.w * vx;
                st.py += s.w * vy;
            }
            None => {
                st.px += s.w * st.tx;
                st.py += s.w * st.ty;
            }
        }
        st.pz += s.w * st.tz;
    }
}

variation! {
    /// `pre_bwraps` — overwrites FT. No fallback: out-of-radius points and a
    /// zero cellsize leave the point completely unmodified.
    PreBwraps, "pre_bwraps", Pass::Pre,
    params {
        "pre_bwraps_cellsize" => cellsize = 1.0, reset = 1.0,
        "pre_bwraps_space" => space = 0.0,
        "pre_bwraps_gain" => gain = 1.0, reset = 1.0,
        "pre_bwraps_inner_twist" => inner_twist = 0.0,
        "pre_bwraps_outer_twist" => outer_twist = 0.0,
    }
    state { g2: f64 = 0.0, r2: f64 = 0.0, rfactor: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (g2, r2, rfactor) = prep(s.cellsize, s.space, s.gain);
        s.g2 = g2;
        s.r2 = r2;
        s.rfactor = rfactor;
    }
    calc |s, st, _rng, _g| {
        if s.cellsize == 0.0 {
            return;
        }
        if let Some((vx, vy)) =
            warp(st.tx, st.ty, s.cellsize, s.inner_twist, s.outer_twist, s.g2, s.r2, s.rfactor)
        {
            st.tx = s.w * vx;
            st.ty = s.w * vy;
            st.tz = s.w * st.tz;
        }
    }
}

variation! {
    /// `post_bwraps` — reads and overwrites FP. Same no-fallback behaviour as
    /// `pre_bwraps`. This is the only variant whose setter guards cellsize.
    PostBwraps, "post_bwraps", Pass::Post,
    params {
        "post_bwraps_cellsize" => cellsize = 1.0,
            coerce = |v: f64| if v == 0.0 { 1e-6 } else { v },
            reset = 1.0,
        "post_bwraps_space" => space = 0.0,
        "post_bwraps_gain" => gain = 1.0, reset = 1.0,
        "post_bwraps_inner_twist" => inner_twist = 0.0,
        "post_bwraps_outer_twist" => outer_twist = 0.0,
    }
    state { g2: f64 = 0.0, r2: f64 = 0.0, rfactor: f64 = 0.0 }
    prepare |s, _w, _c, _rng| {
        let (g2, r2, rfactor) = prep(s.cellsize, s.space, s.gain);
        s.g2 = g2;
        s.r2 = r2;
        s.rfactor = rfactor;
    }
    calc |s, st, _rng, _g| {
        if s.cellsize == 0.0 {
            return;
        }
        if let Some((vx, vy)) =
            warp(st.px, st.py, s.cellsize, s.inner_twist, s.outer_twist, s.g2, s.r2, s.rfactor)
        {
            st.px = s.w * vx;
            st.py = s.w * vy;
            st.pz = s.w * st.pz;
        }
    }
}

pub const NAMES: [&str; 3] = ["bwraps", "pre_bwraps", "post_bwraps"];

pub fn create(name: &str) -> Option<Box<dyn crate::variation::Variation>> {
    match name {
        "bwraps" => Some(Box::new(Bwraps::default())),
        "pre_bwraps" => Some(Box::new(PreBwraps::default())),
        "post_bwraps" => Some(Box::new(PostBwraps::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{GaussBuf, Rng};
    use crate::variation::Variation;

    /// Input in FT, accumulator zeroed — how `next_point` actually calls a
    /// normal or pre_ variation.
    fn state(x: f64, y: f64) -> VarState {
        VarState { tx: x, ty: y, tz: 1.0, px: 0.0, py: 0.0, pz: 0.0, ..Default::default() }
    }

    /// Post variations read the accumulator, not FT.
    fn state_post(x: f64, y: f64) -> VarState {
        VarState { tx: 0.0, ty: 0.0, tz: 0.0, px: x, py: y, pz: 1.0, ..Default::default() }
    }

    /// A point far outside the cell radius: the base passes it through, the
    /// pre_ and post_ variants must leave it entirely alone.
    #[test]
    fn out_of_radius_fallback_differs_across_family() {
        let mut rng = Rng::new(1);
        let mut g = GaussBuf::new(&mut rng);

        // `space` shrinks the bubble inside the cell: radius =
        // 0.5*cellsize/(1+space^2), so space=2 gives radius 0.1 (r2=0.01),
        // and (0.4,0.4) sits 0.02 from its cell centre — outside.
        let mut base = Bwraps::default();
        base.space = 2.0;
        base.prepare(1.0, &crate::flame::Affine::IDENTITY, &mut rng);
        let mut st = state(0.4, 0.4);
        base.calc(&mut st, &mut rng, &mut g);
        assert!((st.px - 0.4).abs() < 1e-12, "base should pass through: {}", st.px);

        let mut pre = PreBwraps::default();
        pre.space = 2.0;
        pre.prepare(1.0, &crate::flame::Affine::IDENTITY, &mut rng);
        let mut st = state(0.4, 0.4);
        pre.calc(&mut st, &mut rng, &mut g);
        assert_eq!(st.tx, 0.4, "pre_ must leave the point untouched");
        assert_eq!(st.tz, 1.0, "pre_ must not scale z on the fallback path");
    }

    /// cellsize == 0 reaches the base kernel because its setter does not guard.
    #[test]
    fn zero_cellsize_is_reachable_for_base_but_guarded_for_post() {
        let mut rng = Rng::new(2);
        let mut g = GaussBuf::new(&mut rng);

        let mut base = Bwraps::default();
        assert_eq!(base.set_param("bwraps_cellsize", 0.0), Some(0.0), "base must not guard");
        base.prepare(1.0, &crate::flame::Affine::IDENTITY, &mut rng);
        let mut st = state(0.3, 0.2);
        base.calc(&mut st, &mut rng, &mut g);
        assert!((st.px - 0.3).abs() < 1e-12, "zero cellsize should pass through");

        let mut post = PostBwraps::default();
        assert_eq!(
            post.set_param("post_bwraps_cellsize", 0.0),
            Some(1e-6),
            "post_ setter must coerce 0 to 1e-6"
        );

        // post_ reads the accumulator, and out-of-radius leaves it untouched.
        post.space = 2.0;
        post.prepare(1.0, &crate::flame::Affine::IDENTITY, &mut rng);
        let mut st = state_post(0.4, 0.4);
        post.calc(&mut st, &mut rng, &mut g);
        assert_eq!(st.px, 0.4, "post_ must leave an out-of-radius point untouched");
    }

    /// Reset is not uniformly zero: cellsize and gain reset to 1.
    #[test]
    fn reset_values_match_original() {
        let mut v = Bwraps::default();
        v.set_param("bwraps_cellsize", 7.0);
        v.set_param("bwraps_space", 7.0);
        assert_eq!(v.reset_param("bwraps_cellsize"), Some(1.0));
        assert_eq!(v.reset_param("bwraps_gain"), Some(1.0));
        assert_eq!(v.reset_param("bwraps_space"), Some(0.0));
    }
}
