// Ported from Apophysis 7X. See LICENSE (GPL-2.0-or-later) at the repo root.
// Original copyright: Mark Townsend; Ronald Hordijk, Piotr Borys, Peter Sdobnov;
// Peter Sdobnov ("3D hack"); Georg Kiehne ("7X").

use crate::rng::{GaussBuf, Rng};

/// Working state threaded through one xform application.
///
/// Mirrors `TXForm`'s `FTx/FTy/FTz` (variation input) and `FPx/FPy/FPz`
/// (accumulator), plus the shared precalcs and the plugin-visible colour.
/// The Delphi version wired these up as raw `^double` pointers into the owning
/// xform (`TBaseVariation.FTx` et al); a struct of values is the same data with
/// the aliasing made explicit.
#[derive(Clone, Copy, Default)]
pub struct VarState {
    /// Variation input, post-affine. `pre_*` variations mutate these.
    pub tx: f64,
    pub ty: f64,
    pub tz: f64,
    /// Accumulator. Normal variations *add* into these; `post_*` mutate them.
    pub px: f64,
    pub py: f64,
    pub pz: f64,
    /// Plugin-visible colour coordinate (`TXForm.vc`).
    pub vc: f64,

    /// The owning xform's affine coefficients, exposed to variations that
    /// read them (`TBaseVariation.a..f`).
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,

    // Shared precalcs, filled by the precalc node that `prepare` splices in
    // between the pre_ and normal passes.
    /// NOTE: the original computes `arctan2(FTx, FTy)` — arguments *swapped*
    /// relative to the conventional `atan2(y, x)`. Preserved deliberately;
    /// "fixing" it rotates and mirrors every polar-family variation.
    pub angle: f64,
    pub length: f64,
    pub sin_a: f64,
    pub cos_a: f64,
}

/// Smallest denominator guard used throughout the original (`EPS = 1E-300`).
pub const EPS: f64 = 1e-300;

/// Which pass a variation runs in.
///
/// `TXForm.Prepare` builds its calc list in four passes: `pre_*` names first,
/// then a shared precalc node, then normal variations, then `post_*` names
/// **and `flatten`**. `flatten` is a normal-looking name that runs in the post
/// pass, which is why this is an explicit enum rather than a name prefix test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    Pre,
    Normal,
    Post,
}

/// Which shared precalcs a variation needs.
///
/// The original picks one of `PrecalcAll` / `PrecalcAngle` / `PrecalcSinCos`
/// based on which builtin indices are active:
///   `CalculateAngle  := (vars[6] <> 0) or (vars[7] <> 0)`   // polar, disc
///   `CalculateSinCos := (vars[8] <> 0) or (vars[10] <> 0)`  // spiral, diamond
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Precalc {
    #[default]
    None,
    Angle,
    SinCos,
    All,
}

/// A variation instance: its parameters plus whatever `prepare` precomputed.
///
/// Corresponds to `TBaseVariation`. The weight (`vvar`) is injected at
/// `prepare` time and stored on the instance, exactly as the original does.
pub trait Variation: VariationClone + Send {
    fn name(&self) -> &'static str;

    fn pass(&self) -> Pass {
        Pass::Normal
    }

    fn precalc(&self) -> Precalc {
        Precalc::None
    }

    /// Called once per render setup, after weight and affine coefs are known.
    fn prepare(&mut self, weight: f64, coefs: &crate::flame::Affine, rng: &mut Rng);

    /// The hot path. Reads `st.t*`, writes `st.p*` (or `st.t*` for `Pass::Pre`).
    fn calc(&self, st: &mut VarState, rng: &mut Rng, g: &mut GaussBuf);

    /// Named parameters, e.g. `julian_power`. Names live in one global
    /// namespace across all variations (see `XFormMan.VariableNames`), so they
    /// must be unique.
    fn param_names(&self) -> &'static [&'static str] {
        &[]
    }

    fn get_param(&self, _name: &str) -> Option<f64> {
        None
    }

    /// Returns the value actually stored, which may be coerced — several
    /// variations clamp or reject (e.g. `julian_power` rounds and forbids 0).
    /// The original signals this via a `var` parameter written back in place.
    fn set_param(&mut self, _name: &str, _value: f64) -> Option<f64> {
        None
    }

    /// Reset to default. Note this is *not* always zero: several variations
    /// use an idiom where reset toggles a sign (`julian_power` flips 2/-2).
    fn reset_param(&mut self, name: &str) -> Option<f64> {
        self.set_param(name, 0.0)
    }
}

/// Enables `Box<dyn Variation>` cloning, needed to hand each worker thread its
/// own instance (the original clones the whole control point per thread).
pub trait VariationClone {
    fn clone_box(&self) -> Box<dyn Variation>;
}

impl<T> VariationClone for T
where
    T: 'static + Variation + Clone,
{
    fn clone_box(&self) -> Box<dyn Variation> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Variation> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl VarState {
    /// The precalc node spliced between the pre_ and normal passes.
    #[inline(always)]
    pub fn run_precalc(&mut self, which: Precalc) {
        match which {
            Precalc::None => {}
            Precalc::Angle => {
                self.angle = self.tx.atan2(self.ty);
            }
            Precalc::SinCos => {
                self.length = (self.tx * self.tx + self.ty * self.ty).sqrt() + EPS;
                self.sin_a = self.tx / self.length;
                self.cos_a = self.ty / self.length;
            }
            Precalc::All => {
                self.angle = self.tx.atan2(self.ty);
                self.length = (self.tx * self.tx + self.ty * self.ty).sqrt() + EPS;
                self.sin_a = self.tx / self.length;
                self.cos_a = self.ty / self.length;
            }
        }
    }
}
