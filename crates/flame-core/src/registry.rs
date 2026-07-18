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

/// Every variation name known to this build, built-ins first.
pub fn all_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = BUILTIN_NAMES.to_vec();
    names.extend(plugins::names());
    names
}

/// Resolve the aliases the original accepts on read but never writes.
///
/// `Main.pas:CreateSubstMap` builds these pairs and `ReadWithSubst` tries the
/// canonical name first, then the alias. Writing always uses the canonical
/// name, so this is read-side only.
pub fn canonical_name(name: &str) -> &str {
    match name {
        "cross2" => "cross",
        "bwraps2" => "bwraps",
        "pre_bwraps2" => "pre_bwraps",
        "post_bwraps2" => "post_bwraps",
        other => {
            // The Epispiral family aliases differ only by leading capital.
            if let Some(rest) = other.strip_prefix("Epispiral") {
                // "Epispiral" -> "epispiral", "EpispiralWedge" -> "epispiralwedge"
                return match rest {
                    "" => "epispiral",
                    _ => other,
                };
            }
            other
        }
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
}
