// Ported from Apophysis 7X `src/Flame/XForm.pas` and `src/Flame/ControlPoint.pas`.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::rng::{GaussBuf, Rng};
use crate::variation::{Pass, Precalc, VarState, Variation};

/// 3x2 affine transform, stored in flam3's `coefs="a b c d e f"` order.
///
/// Delphi lays this out as `c[0..2, 0..1]`, i.e.
/// `c[0,0]=a c[0,1]=b  c[1,0]=c c[1,1]=d  c[2,0]=e c[2,1]=f`,
/// applied as `x' = a*x + c*y + e`, `y' = b*x + d*y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine =
        Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }

    #[inline(always)]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
}

/// A point in the chaos game. `o` is opacity, carried so the plot stage can
/// stochastically reject (`if random >= o then continue`).
#[derive(Clone, Copy, Default, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub c: f64,
    pub o: f64,
}

/// Maximum transform count. The original fixes `NXFORMS = 100`.
pub const NXFORMS: usize = 100;

/// One transform.
pub struct XForm {
    /// Affine coefficients (XML `coefs`).
    pub coefs: Affine,
    /// Post transform (XML `post`). Identity by default; note it does **not**
    /// touch z.
    pub post: Affine,
    /// Selection weight (XML `weight`).
    pub density: f64,
    /// Colour coordinate in [0, 1] (XML `color`).
    pub color: f64,
    /// Colour speed (XML `symmetry`, also accepted as `color_speed`).
    pub symmetry: f64,
    /// Stochastic plot opacity in [0, 1] (XML `opacity`).
    pub opacity: f64,
    /// Blend factor for a variation-supplied colour (XML `var_color`).
    pub plugin_color: f64,
    /// Xaos row: transition weight multipliers to each other xform (XML `chaos`).
    pub mod_weights: Vec<f64>,
    pub name: String,

    /// The authored variation set: every variation the user has attached,
    /// paired with its weight, including zero-weighted ones. This is the
    /// editable surface and survives re-preparation.
    vars: Vec<(Box<dyn Variation>, f64)>,
    /// Nonzero-weighted variations sorted into calc order, rebuilt by
    /// `prepare`. Corresponds to `FCalcFunctionList`.
    calc_list: Vec<Box<dyn Variation>>,
    /// Index into `calc_list` where the normal pass begins — the precalc node
    /// runs at this boundary.
    normal_start: usize,
    /// Which shared precalcs the normal pass needs.
    precalc: Precalc,
    /// Whether to append the post-affine step.
    has_post: bool,

    // Colour blend constants, from `Prepare`:
    //   colorC1 := (1 + symmetry) / 2
    //   colorC2 := color * (1 - symmetry) / 2
    color_c1: f64,
    color_c2: f64,
}

impl Clone for XForm {
    fn clone(&self) -> Self {
        XForm {
            coefs: self.coefs,
            post: self.post,
            density: self.density,
            color: self.color,
            symmetry: self.symmetry,
            opacity: self.opacity,
            plugin_color: self.plugin_color,
            mod_weights: self.mod_weights.clone(),
            name: self.name.clone(),
            vars: self.vars.clone(),
            calc_list: self.calc_list.clone(),
            normal_start: self.normal_start,
            precalc: self.precalc,
            has_post: self.has_post,
            color_c1: self.color_c1,
            color_c2: self.color_c2,
        }
    }
}

impl Default for XForm {
    fn default() -> Self {
        XForm {
            coefs: Affine::IDENTITY,
            post: Affine::IDENTITY,
            density: 0.0,
            color: 0.0,
            symmetry: 0.0,
            opacity: 1.0,
            plugin_color: 1.0,
            mod_weights: vec![1.0; NXFORMS],
            name: String::new(),
            vars: Vec::new(),
            calc_list: Vec::new(),
            normal_start: 0,
            precalc: Precalc::None,
            has_post: false,
            color_c1: 0.5,
            color_c2: 0.0,
        }
    }
}

impl XForm {
    /// Install the variation set. Takes every variation regardless of weight;
    /// `prepare` drops the zero-weighted ones, matching the original's
    /// "skip if `vars[i] = 0`" test in each of its four passes.
    pub fn set_variations(&mut self, vars: Vec<(Box<dyn Variation>, f64)>) {
        self.vars = vars;
    }

    pub fn variations(&self) -> &[(Box<dyn Variation>, f64)] {
        &self.vars
    }

    /// Attach one variation, replacing any existing entry with the same name.
    pub fn set_variation(&mut self, var: Box<dyn Variation>, weight: f64) {
        let name = var.name();
        match self.vars.iter_mut().find(|(v, _)| v.name() == name) {
            Some(slot) => *slot = (var, weight),
            None => self.vars.push((var, weight)),
        }
    }

    /// Set a named parameter on an attached variation, e.g.
    /// `set_variation_param("julian", "julian_power", 3.0)`.
    ///
    /// Returns the value actually stored, which may be coerced — several
    /// variations round, clamp, or forbid zero. `None` means the variation is
    /// not attached or does not declare that parameter.
    pub fn set_variation_param(&mut self, var: &str, param: &str, value: f64) -> Option<f64> {
        self.vars
            .iter_mut()
            .find(|(v, _)| v.name() == var)
            .and_then(|(v, _)| v.set_param(param, value))
    }

    /// Read a named parameter from an attached variation.
    pub fn variation_param(&self, var: &str, param: &str) -> Option<f64> {
        self.vars.iter().find(|(v, _)| v.name() == var).and_then(|(v, _)| v.get_param(param))
    }

    /// Adjust a variation's weight in place. Returns false if not attached.
    pub fn set_weight(&mut self, name: &str, weight: f64) -> bool {
        match self.vars.iter_mut().find(|(v, _)| v.name() == name) {
            Some((_, w)) => {
                *w = weight;
                true
            }
            None => false,
        }
    }

    /// Precompute everything the hot loop needs.
    ///
    /// Ordering here reproduces `TXForm.Prepare`'s four-pass build of
    /// `FCalcFunctionList`: pre_ variations, then a precalc node, then normal
    /// variations, then post_ variations and `flatten`, then the post affine.
    pub fn prepare(&mut self, rng: &mut Rng) {
        self.color_c1 = (1.0 + self.symmetry) / 2.0;
        self.color_c2 = self.color * (1.0 - self.symmetry) / 2.0;

        // Clone the authored set, dropping zero-weighted entries. Cloning
        // keeps `self.vars` intact as the editable master copy.
        let mut tagged: Vec<(Box<dyn Variation>, f64)> =
            self.vars.iter().filter(|(_, w)| *w != 0.0).cloned().collect();

        // Order by pass, then by registry index within the pass. The original
        // iterates each pass over the fixed registry (`XForm.pas:344-383`), so
        // within-pass execution order is the registration order, NOT the order
        // the `.flame` file happens to list its attributes in. This matters
        // wherever two same-pass variations don't commute (any two `pre_`s,
        // any two `post_`s, and `flatten` — registry index 1 — which must run
        // before the `post_*` plugins). Files written by Apophysis list
        // attributes in registry order, so the two orders coincide there, but
        // hand-edited files need not.
        let rank = |p: Pass| match p {
            Pass::Pre => 0,
            Pass::Normal => 1,
            Pass::Post => 2,
        };
        tagged.sort_by_key(|(v, _)| {
            (rank(v.pass()), crate::registry::order_index(v.name()).unwrap_or(usize::MAX))
        });

        self.normal_start = tagged.iter().filter(|(v, _)| v.pass() == Pass::Pre).count();

        // Combine precalc requirements. Angle and SinCos are independent flags
        // in the original, so "needs both" must resolve to All rather than to
        // whichever enum variant sorts higher.
        let mut want_angle = false;
        let mut want_sincos = false;
        for (v, _) in tagged.iter() {
            match v.precalc() {
                Precalc::Angle => want_angle = true,
                Precalc::SinCos => want_sincos = true,
                Precalc::All => {
                    want_angle = true;
                    want_sincos = true;
                }
                Precalc::None => {}
            }
        }
        self.precalc = match (want_angle, want_sincos) {
            (true, true) => Precalc::All,
            (true, false) => Precalc::Angle,
            (false, true) => Precalc::SinCos,
            (false, false) => Precalc::None,
        };

        let coefs = self.coefs;
        self.calc_list = tagged
            .into_iter()
            .map(|(mut v, w)| {
                v.prepare(w, &coefs, rng);
                v
            })
            .collect();

        self.has_post = !self.post.is_identity();
    }

    /// Advance a point through this transform.
    ///
    /// Order, straight from `TXForm.NextPoint`:
    ///   1. colour blend        `c := c*colorC1 + colorC2`, exposed as `vc`
    ///   2. affine (2D only)    z passes through untouched
    ///   3. reset accumulator   `Fp* := 0`
    ///   4. pre_ / precalc / normal / post_ variations
    ///   5. post affine         (only when non-identity; also skips z)
    ///   6. plugin colour blend `c += pluginColor * (vc - c)`
    #[inline(always)]
    pub fn next_point(&self, p: &mut Point, rng: &mut Rng, g: &mut GaussBuf) {
        p.c = p.c * self.color_c1 + self.color_c2;

        let mut st = VarState {
            tx: self.coefs.a * p.x + self.coefs.c * p.y + self.coefs.e,
            ty: self.coefs.b * p.x + self.coefs.d * p.y + self.coefs.f,
            tz: p.z,
            px: 0.0,
            py: 0.0,
            pz: 0.0,
            vc: p.c,
            a: self.coefs.a,
            b: self.coefs.b,
            c: self.coefs.c,
            d: self.coefs.d,
            e: self.coefs.e,
            f: self.coefs.f,
            ..Default::default()
        };

        for v in &self.calc_list[..self.normal_start] {
            v.calc(&mut st, rng, g);
        }
        st.run_precalc(self.precalc);
        for v in &self.calc_list[self.normal_start..] {
            v.calc(&mut st, rng, g);
        }

        if self.has_post {
            let tmp = st.px;
            st.px = self.post.a * st.px + self.post.c * st.py + self.post.e;
            st.py = self.post.b * tmp + self.post.d * st.py + self.post.f;
        }

        p.c += self.plugin_color * (st.vc - p.c);
        p.x = st.px;
        p.y = st.py;
        p.z = st.pz;
    }

    /// As `next_point`, but writes to a separate output point without
    /// disturbing the input. Used for the final xform, which must not advance
    /// the orbit state.
    #[inline(always)]
    pub fn next_point_to(&self, src: &Point, dst: &mut Point, rng: &mut Rng, g: &mut GaussBuf) {
        *dst = *src;
        self.next_point(dst, rng, g);
    }

    /// True if this xform differs from a pass-through, i.e. is worth applying
    /// as a final xform. Mirrors `TControlPoint.HasFinalXForm`
    /// (ControlPoint.pas:2320): identity coefs and post, `symmetry = 1`,
    /// `linear = 1`, and every other variation weight 0 means "not used".
    /// The variation test is on *weights*, not on which entries are attached.
    pub fn is_meaningful(&self) -> bool {
        if !self.coefs.is_identity() || !self.post.is_identity() || self.symmetry != 1.0 {
            return true;
        }
        self.vars
            .iter()
            .any(|(v, w)| if v.name() == "linear" { *w != 1.0 } else { *w != 0.0 })
            || !self.vars.iter().any(|(v, w)| v.name() == "linear" && *w == 1.0)
    }
}
