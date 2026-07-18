// Plugin variations — ported from `src/Variations/var*.pas`.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// Each of these corresponds to one `TBaseVariation` subclass. The `variation!`
// macro generates the parts that are identical across all of them (struct,
// defaults, parameter plumbing, trait wiring) so each file carries only the
// parts that actually differ: the precomputation and the math.

#[macro_use]
mod macros;

mod bwraps;
mod crop;
mod curl;
mod falloff2;
mod blur;
mod misc_a;
mod misc_b;
mod misc_c;
mod julia;

use crate::variation::Variation;

/// Delphi's `Math.IsZero` with its default epsilon — an approximate compare,
/// not `== 0`. Several variations dispatch on it, so `c1 = 1e-13` really does
/// take the "zero" branch and produce different output than `c1 = 1e-10`.
#[inline]
pub(crate) fn is_zero(v: f64) -> bool {
    v.abs() <= 1e-12
}

/// Build a plugin variation by name. Returns `None` for unknown names, which
/// the loader surfaces as a missing-plugin warning rather than an error — the
/// original does the same via `MissingPlugin.pas`.
pub fn create(name: &str) -> Option<Box<dyn Variation>> {
    bwraps::create(name)
        .or_else(|| crop::create(name))
        .or_else(|| curl::create(name))
        .or_else(|| falloff2::create(name))
        .or_else(|| julia::create(name))
        .or_else(|| blur::create(name))
        .or_else(|| misc_a::create(name))
        .or_else(|| misc_b::create(name))
        .or_else(|| misc_c::create(name))
}

/// Every plugin variation name in this build.
pub fn names() -> Vec<&'static str> {
    let mut v = Vec::new();
    v.extend(bwraps::NAMES);
    v.extend(crop::NAMES);
    v.extend(curl::NAMES);
    v.extend(falloff2::NAMES);
    v.extend(julia::NAMES);
    v.extend(blur::NAMES);
    v.extend(misc_a::NAMES);
    v.extend(misc_b::NAMES);
    v.extend(misc_c::NAMES);
    v
}

/// Convenience for the registry tests and the UI's variation list.
pub fn count() -> usize {
    names().len()
}

#[allow(unused_imports)]
pub(crate) use crate::flame::Affine;
#[allow(unused_imports)]
pub(crate) use crate::rng::{GaussBuf, Rng};
#[allow(unused_imports)]
pub(crate) use crate::variation::{Pass, Precalc, VarState, EPS};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_name_returns_none() {
        assert!(create("definitely_not_a_variation").is_none());
    }

    #[test]
    fn every_plugin_name_constructs() {
        for name in names() {
            let v: Box<dyn Variation> =
                create(name).unwrap_or_else(|| panic!("no constructor for {name}"));
            assert_eq!(v.name(), name);
        }
    }
}
