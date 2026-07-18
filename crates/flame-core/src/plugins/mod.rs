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

use crate::variation::Variation;

/// Build a plugin variation by name. Returns `None` for unknown names, which
/// the loader surfaces as a missing-plugin warning rather than an error — the
/// original does the same via `MissingPlugin.pas`.
pub fn create(name: &str) -> Option<Box<dyn Variation>> {
    bwraps::create(name)
}

/// Every plugin variation name in this build.
pub fn names() -> Vec<&'static str> {
    let mut v = Vec::new();
    v.extend(bwraps::NAMES);
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
