// Boilerplate generator for plugin variations.
// See LICENSE (GPL-2.0-or-later) at the repo root.

/// Declares one variation.
///
/// Generates the struct, its defaults, the parameter accessors, and the
/// `Variation` impl, leaving only `prepare` and `calc` to be written out.
///
/// `params` are the named values that appear as `.flame` XML attributes
/// (globally unique across all variations — see the registry test). Each may
/// carry a `coerce` expression, applied on set: the original signals coercion
/// by writing back through a `var` parameter, so several variations round to
/// integer, forbid zero, or clamp. `reset` overrides the reset value, which is
/// not always zero — some variations toggle a sign instead.
///
/// `state` holds values computed by `prepare`; they are never serialised.
///
/// ```ignore
/// variation! {
///     Julian, "julian", Pass::Normal,
///     params {
///         "julian_power" => power = 2.0,
///             coerce = |v: f64| { let n = v.round(); if n == 0.0 { 1.0 } else { n } },
///             reset = 2.0,
///         "julian_dist" => dist = 1.0,
///     }
///     state { abs_n: f64 = 0.0, cn: f64 = 0.0 }
///     prepare |s, w, _coefs, _rng| { s.abs_n = s.power.abs(); }
///     calc |s, st, rng, _g| { st.px += s.w; }
/// }
/// ```
macro_rules! variation {
    (
        $(#[$meta:meta])*
        $ty:ident, $name:literal, $pass:expr,
        params {
            $( $pname:literal => $pfield:ident = $pdefault:expr
               $(, coerce = $pcoerce:expr )?
               $(, reset = $preset:expr )?
            ),* $(,)?
        }
        state { $( $sfield:ident : $stype:ty = $sdefault:expr ),* $(,)? }
        $( precalc $precalc:expr; )?
        prepare | $ps:ident, $pw:ident, $pc:ident, $prng:ident | $prep:block
        calc | $cs:ident, $cst:ident, $crng:ident, $cg:ident | $calc:block
    ) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $ty {
            /// `vvar` in the original — this variation's weight on the owning xform.
            pub w: f64,
            $( pub $pfield: f64, )*
            $( $sfield: $stype, )*
        }

        impl Default for $ty {
            fn default() -> Self {
                $ty {
                    w: 0.0,
                    $( $pfield: $pdefault, )*
                    $( $sfield: $sdefault, )*
                }
            }
        }

        impl $crate::variation::Variation for $ty {
            fn name(&self) -> &'static str { $name }

            fn pass(&self) -> $crate::variation::Pass { $pass }

            $( fn precalc(&self) -> $crate::variation::Precalc { $precalc } )?

            fn param_names(&self) -> &'static [&'static str] {
                &[ $( $pname ),* ]
            }

            fn get_param(&self, name: &str) -> Option<f64> {
                match name {
                    $( $pname => Some(self.$pfield), )*
                    _ => None,
                }
            }

            #[allow(unused_variables, clippy::redundant_closure_call)]
            fn set_param(&mut self, name: &str, value: f64) -> Option<f64> {
                match name {
                    $(
                        $pname => {
                            // Coercion mirrors the original writing a corrected
                            // value back through its `var` parameter.
                            let v = value;
                            $( let v = ($pcoerce)(v); )?
                            self.$pfield = v;
                            Some(v)
                        }
                    )*
                    _ => None,
                }
            }

            #[allow(unused_variables)]
            fn reset_param(&mut self, name: &str) -> Option<f64> {
                match name {
                    $(
                        $pname => {
                            #[allow(unused_mut, unused_assignments)]
                            let mut v = 0.0;
                            $( v = $preset; )?
                            self.$pfield = v;
                            Some(v)
                        }
                    )*
                    _ => None,
                }
            }

            #[allow(unused_variables)]
            fn prepare(
                &mut self,
                weight: f64,
                coefs: &$crate::flame::Affine,
                rng: &mut $crate::rng::Rng,
            ) {
                self.w = weight;
                let $ps = self;
                let $pw = weight;
                let $pc = coefs;
                let $prng = rng;
                $prep
            }

            #[allow(unused_variables)]
            #[inline(always)]
            fn calc(
                &self,
                st: &mut $crate::variation::VarState,
                rng: &mut $crate::rng::Rng,
                g: &mut $crate::rng::GaussBuf,
            ) {
                let $cs = self;
                let $cst = st;
                let $crng = rng;
                let $cg = g;
                $calc
            }
        }
    };
}
