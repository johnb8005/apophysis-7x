// Genetic mutation, for the Mutation window's 3x3 exploration grid.
// Ported in spirit from `src/Forms/Mutate.pas` and the randomisation helpers
// in `src/Flame/RndFlame.pas`.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::flame::Affine;
use crate::genome::Flame;
use crate::registry;
use crate::rng::Rng;

/// What kind of change a mutation makes.
///
/// The original offers a "trend" combo that biases mutations toward a chosen
/// variation; `Random` reproduces its default behaviour of picking a strategy
/// per mutant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trend {
    /// Nudge the affine coefficients.
    Coefs,
    /// Nudge selection weights.
    Weights,
    /// Nudge colours and colour speeds.
    Colors,
    /// Adjust the weight of an existing variation.
    VariationWeights,
    /// Attach a randomly chosen variation to one transform.
    AddVariation,
    /// Pick one of the above per mutant.
    Random,
}

impl Trend {
    pub fn from_str(s: &str) -> Trend {
        match s {
            "coefs" => Trend::Coefs,
            "weights" => Trend::Weights,
            "colors" => Trend::Colors,
            "varweights" => Trend::VariationWeights,
            "addvar" => Trend::AddVariation,
            _ => Trend::Random,
        }
    }
}

/// A pool of visually interesting variations for `AddVariation`.
///
/// Deliberately not the full 76: mutating toward `flatten` or a `pre_`/`post_`
/// helper usually produces a worse flame, and the original's random-flame
/// generator likewise draws from a curated set (its Options dialog lets the
/// user choose which variations are eligible).
const MUTATION_POOL: [&str; 26] = [
    "linear",
    "sinusoidal",
    "spherical",
    "swirl",
    "horseshoe",
    "polar",
    "disc",
    "spiral",
    "hyperbolic",
    "diamond",
    "eyefish",
    "bubble",
    "cylinder",
    "julian",
    "juliascope",
    "curl",
    "bipolar",
    "cross",
    "elliptic",
    "escher",
    "foci",
    "loonie",
    "ngon",
    "pdj",
    "rings2",
    "waves2",
];

/// Produce a mutated copy.
///
/// `amount` scales every perturbation; the UI exposes it as the original's
/// "Speed" scrollbar. Mutation never changes the transform count, matching the
/// original's "same number of transforms" default — that keeps the grid's
/// mutants recognisably related to the parent.
pub fn mutate(flame: &Flame, trend: Trend, amount: f64, seed: u64) -> Flame {
    let mut out = flame.clone();
    let mut rng = Rng::new(seed);

    if out.xforms.is_empty() {
        return out;
    }

    let trend = if trend == Trend::Random {
        match rng.below(5) {
            0 => Trend::Coefs,
            1 => Trend::Weights,
            2 => Trend::Colors,
            3 => Trend::VariationWeights,
            _ => Trend::AddVariation,
        }
    } else {
        trend
    };

    let n = out.xforms.len();
    let pick = rng.below(n as u32) as usize;

    match trend {
        Trend::Coefs => {
            // Perturb every transform a little, and the picked one more, so a
            // mutant reads as a variation on the parent rather than noise.
            for (i, xf) in out.xforms.iter_mut().enumerate() {
                let k = if i == pick { amount } else { amount * 0.35 };
                let j = |rng: &mut Rng| rng.f64_signed() * k;
                xf.coefs = Affine {
                    a: xf.coefs.a + j(&mut rng),
                    b: xf.coefs.b + j(&mut rng),
                    c: xf.coefs.c + j(&mut rng),
                    d: xf.coefs.d + j(&mut rng),
                    e: xf.coefs.e + j(&mut rng) * 0.6,
                    f: xf.coefs.f + j(&mut rng) * 0.6,
                };
            }
        }

        Trend::Weights => {
            for xf in out.xforms.iter_mut() {
                // Multiplicative, so a weight cannot walk to zero and silently
                // remove a transform from the chain.
                let factor = 1.0 + rng.f64_signed() * amount;
                xf.density = (xf.density * factor.max(0.05)).clamp(0.001, 100.0);
            }
        }

        Trend::Colors => {
            for xf in out.xforms.iter_mut() {
                xf.color = (xf.color + rng.f64_signed() * amount).rem_euclid(1.0);
                if rng.f64() < 0.3 {
                    xf.symmetry = (xf.symmetry + rng.f64_signed() * amount).clamp(-1.0, 1.0);
                }
            }
        }

        Trend::VariationWeights => {
            let xf = &mut out.xforms[pick];
            let names: Vec<&'static str> =
                xf.variations().iter().map(|(v, _)| v.name()).collect();
            if names.is_empty() {
                if let Some(v) = registry::create("linear") {
                    xf.set_variation(v, 1.0);
                }
            } else {
                let target = names[rng.below(names.len() as u32) as usize];
                let current = xf
                    .variations()
                    .iter()
                    .find(|(v, _)| v.name() == target)
                    .map(|(_, w)| *w)
                    .unwrap_or(0.0);
                xf.set_weight(target, current + rng.f64_signed() * amount);
            }
        }

        Trend::AddVariation => {
            let name = MUTATION_POOL[rng.below(MUTATION_POOL.len() as u32) as usize];
            if let Some(v) = registry::create(name) {
                let weight = rng.f64() * amount * 2.0;
                out.xforms[pick].set_variation(v, weight);
            }
        }

        Trend::Random => unreachable!("resolved above"),
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame::XForm;
    use crate::variation::Variation;

    fn sample() -> Flame {
        let mut f = Flame::default();
        for i in 0..3 {
            let mut xf = XForm::default();
            xf.density = 1.0;
            xf.color = i as f64 / 3.0;
            xf.set_variations(vec![(
                registry::create("linear").unwrap() as Box<dyn Variation>,
                1.0,
            )]);
            f.xforms.push(xf);
        }
        f
    }

    #[test]
    fn mutation_preserves_transform_count() {
        let f = sample();
        for seed in 0..20u64 {
            let m = mutate(&f, Trend::Random, 0.3, seed);
            assert_eq!(m.xforms.len(), f.xforms.len(), "seed {seed} changed the count");
        }
    }

    #[test]
    fn mutation_is_deterministic_for_a_seed() {
        let f = sample();
        let a = mutate(&f, Trend::Coefs, 0.3, 42);
        let b = mutate(&f, Trend::Coefs, 0.3, 42);
        assert_eq!(a.xforms[0].coefs.a, b.xforms[0].coefs.a);
    }

    #[test]
    fn different_seeds_give_different_mutants() {
        let f = sample();
        let a = mutate(&f, Trend::Coefs, 0.3, 1);
        let b = mutate(&f, Trend::Coefs, 0.3, 2);
        assert_ne!(a.xforms[0].coefs.a, b.xforms[0].coefs.a);
    }

    /// Weights are perturbed multiplicatively so a transform cannot be
    /// silently dropped out of the chain.
    #[test]
    fn weight_mutation_never_reaches_zero() {
        let f = sample();
        for seed in 0..50u64 {
            let m = mutate(&f, Trend::Weights, 0.95, seed);
            for xf in &m.xforms {
                assert!(xf.density > 0.0, "seed {seed} produced a zero weight");
            }
        }
    }

    #[test]
    fn colour_stays_in_range() {
        let f = sample();
        for seed in 0..50u64 {
            let m = mutate(&f, Trend::Colors, 0.9, seed);
            for xf in &m.xforms {
                assert!(
                    (0.0..1.0).contains(&xf.color),
                    "seed {seed} colour out of range: {}",
                    xf.color
                );
                assert!((-1.0..=1.0).contains(&xf.symmetry));
            }
        }
    }

    #[test]
    fn add_variation_attaches_something_renderable() {
        let f = sample();
        let m = mutate(&f, Trend::AddVariation, 0.5, 7);
        let total: usize = m.xforms.iter().map(|x| x.variations().len()).sum();
        assert!(total >= 3, "expected at least the original variations, got {total}");
    }
}
