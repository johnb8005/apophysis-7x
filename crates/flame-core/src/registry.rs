// Variation registry — the port of `src/Core/XFormMan.pas`.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::builtins::{Builtin, BUILTIN_NAMES};
use crate::plugins;
use crate::variation::Variation;

/// Construct a variation by its `.flame` XML attribute name.
///
/// The original resolves names by scanning its registry **backwards**
/// (`XFormMan.GetVariationIndex`), so a later registration shadows an earlier
/// duplicate. We instead forbid duplicates outright — see the
/// `no_duplicate_variation_names` test — which is the same behaviour for every
/// well-formed registry and catches collisions at test time rather than
/// silently picking a winner.
pub fn create(name: &str) -> Option<Box<dyn Variation>> {
    if let Some(kind) = Builtin::from_name(name) {
        return Some(Box::new(crate::builtins::BuiltinVar::new(kind)));
    }
    plugins::create(name)
}

/// Plugin variations in the order the original registers them — the
/// `Apophysis7X.dpr` uses-clause order, because each `var*.pas` unit's
/// `initialization` section calls `RegisterVariation`. This order is
/// behaviour-bearing: each calculation pass iterates variations by registry
/// index (`XForm.pas:344-383`), so two non-commuting variations in the same
/// pass must run in this order no matter how the `.flame` file lists them.
pub const PLUGIN_REGISTRY_ORDER: [&str; 47] = [
    "hemisphere",
    "log",
    "polar2",
    "rings2",
    "fan2",
    "cross",
    "wedge",
    "epispiral",
    "bwraps",
    "pdj",
    "julian",
    "juliascope",
    "julia3D",
    "julia3Dz",
    "curl",
    "curl3D",
    "radial_blur",
    "blur_circle",
    "blur_zoom",
    "blur_pixelize",
    "falloff2",
    "rectangles",
    "splits",
    "separation",
    "bipolar",
    "loonie",
    "escher",
    "scry",
    "ngon",
    "foci",
    "lazysusan",
    "mobius",
    "crop",
    "elliptic",
    "waves2",
    "auger",
    "pre_spherical",
    "pre_sinusoidal",
    "pre_disc",
    "pre_bwraps",
    "pre_crop",
    "pre_falloff2",
    "post_bwraps",
    "post_curl",
    "post_curl3D",
    "post_crop",
    "post_falloff2",
];

/// Every variation name known to this build, in registry order: built-ins
/// 0..28 first, then plugins in registration order. Index in this list is the
/// variation's registry index.
pub fn all_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = BUILTIN_NAMES.to_vec();
    names.extend(PLUGIN_REGISTRY_ORDER);
    names
}

/// The registry index of a variation name, used to order execution within a
/// calculation pass. `None` for unknown names.
pub fn order_index(name: &str) -> Option<usize> {
    if let Some(i) = BUILTIN_NAMES.iter().position(|n| *n == name) {
        return Some(i);
    }
    PLUGIN_REGISTRY_ORDER.iter().position(|n| *n == name).map(|i| BUILTIN_NAMES.len() + i)
}

/// Resolve the aliases the original accepts on read but never writes.
///
/// `Main.pas:CreateSubstMap` builds these pairs and `ReadWithSubst` tries the
/// canonical name first, then the alias. Writing always uses the canonical
/// name, so this is read-side only.
pub fn canonical_name(name: &str) -> &str {
    // The full CreateSubstMap table (Main.pas:6971-7004), variation names AND
    // parameter names. Dropping the parameter rows is a silent-data-loss bug:
    // a legacy `bwraps2` flame would load the variation but keep every
    // parameter at its default.
    match name {
        "cross2" => "cross",
        "Epispiral" => "epispiral",
        "Epispiral_n" => "epispiral_n",
        "Epispiral_thickness" => "epispiral_thickness",
        "Epispiral_holes" => "epispiral_holes",
        "bwraps2" | "bwraps7" => "bwraps",
        "bwraps2_cellsize" | "bwraps7_cellsize" => "bwraps_cellsize",
        "bwraps2_space" | "bwraps7_space" => "bwraps_space",
        "bwraps2_gain" | "bwraps7_gain" => "bwraps_gain",
        "bwraps2_inner_twist" | "bwraps7_inner_twist" => "bwraps_inner_twist",
        "bwraps2_outer_twist" | "bwraps7_outer_twist" => "bwraps_outer_twist",
        "pre_bwraps2" => "pre_bwraps",
        "pre_bwraps2_cellsize" => "pre_bwraps_cellsize",
        "pre_bwraps2_space" => "pre_bwraps_space",
        "pre_bwraps2_gain" => "pre_bwraps_gain",
        "pre_bwraps2_inner_twist" => "pre_bwraps_inner_twist",
        "pre_bwraps2_outer_twist" => "pre_bwraps_outer_twist",
        "post_bwraps2" => "post_bwraps",
        "post_bwraps2_cellsize" => "post_bwraps_cellsize",
        "post_bwraps2_space" => "post_bwraps_space",
        "post_bwraps2_gain" => "post_bwraps_gain",
        "post_bwraps2_inner_twist" => "post_bwraps_inner_twist",
        "post_bwraps2_outer_twist" => "post_bwraps_outer_twist",
        "logn" => "log",
        "logn_base" => "log_base",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn no_duplicate_variation_names() {
        let names = all_names();
        let mut seen = HashSet::new();
        for n in &names {
            assert!(seen.insert(*n), "duplicate variation name: {n}");
        }
    }

    #[test]
    fn every_registered_name_constructs() {
        for name in all_names() {
            let v = create(name).unwrap_or_else(|| panic!("failed to construct {name}"));
            assert_eq!(v.name(), name, "variation reports a different name");
        }
    }

    /// Variable names live in one global namespace across all variations
    /// (`XFormMan.VariableNames`), so a collision would make `set_param`
    /// ambiguous — the original resolves it by first-match, silently.
    #[test]
    fn no_duplicate_parameter_names() {
        let mut seen: HashSet<&str> = HashSet::new();
        for name in all_names() {
            let v = create(name).unwrap();
            for p in v.param_names() {
                assert!(
                    seen.insert(p),
                    "parameter name {p} is declared by more than one variation (last: {name})"
                );
            }
        }
    }
}

#[cfg(test)]
mod count_tests {
    use super::*;

    /// 29 built-ins plus the 47 plugin variations ported from src/Variations/.
    #[test]
    fn full_variation_count() {
        assert_eq!(crate::builtins::BUILTIN_NAMES.len(), 29, "builtin count");
        assert_eq!(crate::plugins::count(), 47, "plugin count");
        assert_eq!(all_names().len(), 76, "total");
    }

    /// The registration-order table must cover exactly the built plugin set —
    /// a plugin added to a module but not to the order table would silently
    /// sort last instead of at its Delphi registry position.
    #[test]
    fn registry_order_is_a_permutation_of_the_plugin_set() {
        let mut ordered: Vec<&str> = PLUGIN_REGISTRY_ORDER.to_vec();
        let mut built: Vec<&str> = crate::plugins::names();
        ordered.sort_unstable();
        built.sort_unstable();
        assert_eq!(ordered, built);
    }

    #[test]
    fn order_index_resolves_every_name() {
        for (i, name) in all_names().iter().enumerate() {
            assert_eq!(order_index(name), Some(i), "{name}");
        }
        assert_eq!(order_index("nope"), None);
    }
}
