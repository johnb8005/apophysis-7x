// flame-core — fractal flame renderer ported from Apophysis 7X.
//
// Apophysis Copyright (C) 2001-2004 Mark Townsend
// Apophysis Copyright (C) 2005-2006 Ronald Hordijk, Piotr Borys, Peter Sdobnov
// Apophysis Copyright (C) 2007-2008 Piotr Borys, Peter Sdobnov
// Apophysis "3D hack" Copyright (C) 2007-2008 Peter Sdobnov
// Apophysis "7X" Copyright (C) 2009-2010 Georg Kiehne
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation; either version 2 of the License, or (at your option)
// any later version. See LICENSE at the repository root.

pub mod builtins;
pub mod genome;
pub mod plugins;
pub mod registry;
pub mod render;
pub mod wasm;
pub mod xml;
pub mod flame;
pub mod rng;
pub mod variation;

#[cfg(test)]
mod tests {
    use super::builtins::{Builtin, BuiltinVar, BUILTIN_NAMES};
    use super::flame::{Affine, Point, XForm};
    use super::rng::{GaussBuf, Rng};
    use super::variation::Variation;

    #[test]
    fn builtin_index_matches_name_table() {
        for (i, name) in BUILTIN_NAMES.iter().enumerate() {
            assert_eq!(Builtin::from_index(i) as usize, i);
            assert_eq!(Builtin::from_index(i).name(), *name);
            assert_eq!(Builtin::from_name(name), Some(Builtin::from_index(i)));
        }
    }

    /// A linear xform with weight 1 and identity affine must be a no-op on
    /// position, and must halve the colour toward `color` when symmetry = 0.
    #[test]
    fn linear_identity_is_position_preserving() {
        let mut xf = XForm::default();
        xf.density = 1.0;
        xf.color = 1.0;
        xf.set_variations(vec![(Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0)]);
        let mut rng = Rng::new(1);
        xf.prepare(&mut rng);

        let mut g = GaussBuf::new(&mut rng);
        let mut p = Point { x: 0.3, y: -0.7, z: 0.5, c: 0.0, o: 1.0 };
        xf.next_point(&mut p, &mut rng, &mut g);

        assert!((p.x - 0.3).abs() < 1e-12, "x moved: {}", p.x);
        assert!((p.y + 0.7).abs() < 1e-12, "y moved: {}", p.y);
        assert!((p.z - 0.5).abs() < 1e-12, "z moved: {}", p.z);
        // c' = c*(1+s)/2 + color*(1-s)/2 = 0*0.5 + 1*0.5 = 0.5
        assert!((p.c - 0.5).abs() < 1e-12, "colour: {}", p.c);
    }

    /// `flatten` carries no weight and must run in the post pass, zeroing z
    /// after the normal variations have written it.
    #[test]
    fn flatten_runs_after_normal_pass() {
        let mut xf = XForm::default();
        xf.density = 1.0;
        xf.set_variations(vec![
            (Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0),
            (Box::new(BuiltinVar::new(Builtin::Flatten)) as Box<dyn Variation>, 1.0),
        ]);
        let mut rng = Rng::new(2);
        xf.prepare(&mut rng);

        let mut g = GaussBuf::new(&mut rng);
        let mut p = Point { x: 0.1, y: 0.2, z: 9.0, c: 0.0, o: 1.0 };
        xf.next_point(&mut p, &mut rng, &mut g);
        assert_eq!(p.z, 0.0, "flatten did not zero z");
        assert!((p.x - 0.1).abs() < 1e-12);
    }

    /// The post affine must not touch z (`DoPostTransform` writes x and y only).
    #[test]
    fn post_affine_leaves_z_alone() {
        let mut xf = XForm::default();
        xf.density = 1.0;
        xf.post = Affine { a: 2.0, b: 0.0, c: 0.0, d: 2.0, e: 1.0, f: 0.0 };
        xf.set_variations(vec![(Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0)]);
        let mut rng = Rng::new(3);
        xf.prepare(&mut rng);

        let mut g = GaussBuf::new(&mut rng);
        let mut p = Point { x: 1.0, y: 1.0, z: 4.0, c: 0.0, o: 1.0 };
        xf.next_point(&mut p, &mut rng, &mut g);
        assert!((p.x - 3.0).abs() < 1e-12, "x: {}", p.x);
        assert!((p.y - 2.0).abs() < 1e-12, "y: {}", p.y);
        assert!((p.z - 4.0).abs() < 1e-12, "post transform touched z: {}", p.z);
    }

    /// Re-preparing after a weight change must not corrupt the authored set.
    #[test]
    fn prepare_is_idempotent_over_edits() {
        let mut xf = XForm::default();
        xf.density = 1.0;
        xf.set_variations(vec![(Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0)]);
        let mut rng = Rng::new(4);

        for _ in 0..5 {
            xf.prepare(&mut rng);
            assert_eq!(xf.variations().len(), 1, "authored set was consumed");
        }

        assert!(xf.set_weight("linear", 0.0));
        xf.prepare(&mut rng);
        // Zero-weighted variations drop out of the calc list but stay authored.
        assert_eq!(xf.variations().len(), 1);

        let mut g = GaussBuf::new(&mut rng);
        let mut p = Point { x: 1.0, y: 1.0, z: 1.0, c: 0.0, o: 1.0 };
        xf.next_point(&mut p, &mut rng, &mut g);
        assert_eq!((p.x, p.y), (0.0, 0.0), "zero-weight linear still contributed");
    }

    /// `atan2(x, y)` — argument order is swapped relative to convention, and
    /// polar depends on it. Guard against a well-meaning "fix".
    #[test]
    fn polar_uses_swapped_atan2() {
        let mut xf = XForm::default();
        xf.density = 1.0;
        xf.set_variations(vec![(Box::new(BuiltinVar::new(Builtin::Polar)) as Box<dyn Variation>, 1.0)]);
        let mut rng = Rng::new(5);
        xf.prepare(&mut rng);

        let mut g = GaussBuf::new(&mut rng);
        let mut p = Point { x: 1.0, y: 0.0, z: 0.0, c: 0.0, o: 1.0 };
        xf.next_point(&mut p, &mut rng, &mut g);
        // atan2(x=1, y=0) = pi/2, so px = (1/pi)*(pi/2) = 0.5.
        // The conventional atan2(y=0, x=1) = 0 would give px = 0.
        assert!((p.x - 0.5).abs() < 1e-12, "expected 0.5, got {}", p.x);
    }
}
