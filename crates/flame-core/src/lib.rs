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
pub mod load;
pub mod mutate;
pub mod plugins;
pub mod registry;
pub mod save;
pub mod render;
pub mod wasm;
pub mod xml;
pub mod curves;
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

    /// Within a pass, execution order is the registry order, not the order
    /// variations were attached in (`XForm.pas:344-383` iterates by registry
    /// index). pre_spherical registers before pre_sinusoidal in the original,
    /// and the two don't commute, so both attachment orders must produce the
    /// spherical-then-sinusoidal composition.
    #[test]
    fn within_pass_order_is_registry_order_not_attachment_order() {
        let run = |names: [&str; 2]| {
            let mut xf = XForm::default();
            xf.density = 1.0;
            xf.set_variations(vec![
                (crate::registry::create(names[0]).unwrap(), 1.0),
                (crate::registry::create(names[1]).unwrap(), 1.0),
                (Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0),
            ]);
            let mut rng = Rng::new(7);
            xf.prepare(&mut rng);
            let mut g = GaussBuf::new(&mut rng);
            let mut p = Point { x: 0.3, y: -0.6, z: 0.0, c: 0.0, o: 1.0 };
            xf.next_point(&mut p, &mut rng, &mut g);
            (p.x, p.y)
        };

        let a = run(["pre_spherical", "pre_sinusoidal"]);
        let b = run(["pre_sinusoidal", "pre_spherical"]);
        assert_eq!(a, b, "attachment order changed the composition");

        // And the composition really is spherical first: applying them by
        // hand in that order must reproduce the xform's output.
        let (x0, y0) = (0.3f64, -0.6f64);
        let r2 = x0 * x0 + y0 * y0 + 1.0e-5; // pre_spherical's 10e-6 guard
        let (sx, sy) = (x0 / r2, y0 / r2);
        let expect = (sx.sin(), sy.sin());
        assert!((a.0 - expect.0).abs() < 1e-12, "{} vs {}", a.0, expect.0);
        assert!((a.1 - expect.1).abs() < 1e-12, "{} vs {}", a.1, expect.1);
    }

    /// `HasFinalXForm` (ControlPoint.pas:2320): a final xform is "not there"
    /// exactly when coefs and post are identity, symmetry is 1, linear is 1,
    /// and every other weight is 0. The test is on weights, not attachment.
    #[test]
    fn is_meaningful_mirrors_has_final_xform() {
        let linear1 = || (Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 1.0);

        let mut trivial = XForm::default();
        trivial.symmetry = 1.0;
        trivial.set_variations(vec![linear1()]);
        assert!(!trivial.is_meaningful(), "identity + symmetry 1 + linear 1 is a pass-through");

        // A zero-weighted extra variation doesn't make it meaningful.
        let mut with_zero = trivial.clone();
        with_zero.set_variations(vec![
            linear1(),
            (Box::new(BuiltinVar::new(Builtin::Swirl)) as Box<dyn Variation>, 0.0),
        ]);
        assert!(!with_zero.is_meaningful());

        // Each departure from the pass-through state makes it meaningful.
        let mut coefs = trivial.clone();
        coefs.coefs.e = 0.5;
        assert!(coefs.is_meaningful());

        let mut sym = trivial.clone();
        sym.symmetry = 0.0;
        assert!(sym.is_meaningful());

        let mut weighted = trivial.clone();
        weighted.set_variations(vec![
            linear1(),
            (Box::new(BuiltinVar::new(Builtin::Swirl)) as Box<dyn Variation>, 0.3),
        ]);
        assert!(weighted.is_meaningful());

        let mut scaled_linear = trivial.clone();
        scaled_linear
            .set_variations(vec![(Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>, 0.5)]);
        assert!(scaled_linear.is_meaningful());

        // No variations at all: linear is 0, not 1, so Delphi applies it
        // (and both engines collapse the point) — meaningful.
        let mut empty = trivial.clone();
        empty.set_variations(vec![]);
        assert!(empty.is_meaningful());
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
