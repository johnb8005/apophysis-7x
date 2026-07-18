// Ported from Apophysis 7X `src/Flame/XForm.pas` (BuildFunctionlist + the
// variation methods) and `src/Core/XFormMan.pas` (cvarnames).
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::flame::Affine;
use crate::rng::{GaussBuf, Rng};
use crate::variation::{Pass, Precalc, VarState, Variation, EPS};

/// The 29 variations compiled into `XForm.pas` (`NRLOCVAR = 29`).
///
/// Order is load-bearing: the index here must equal the index in
/// `XFormMan.cvarnames`, because the legacy binary and `vars N N N...` text
/// formats address variations positionally. The XML format uses names, so a
/// pure-XML port could reorder — but the indices also appear in `Prepare`'s
/// precalc test (`vars[6]`, `vars[7]`, `vars[8]`, `vars[10]`), so we keep them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Builtin {
    Linear = 0,
    Flatten = 1,
    Sinusoidal = 2,
    Spherical = 3,
    Swirl = 4,
    Horseshoe = 5,
    Polar = 6,
    Disc = 7,
    Spiral = 8,
    Hyperbolic = 9,
    Diamond = 10,
    Eyefish = 11,
    Bubble = 12,
    Cylinder = 13,
    Noise = 14,
    Blur = 15,
    GaussianBlur = 16,
    ZBlur = 17,
    Blur3D = 18,
    PreBlur = 19,
    PreZScale = 20,
    PreZTranslate = 21,
    PreRotateX = 22,
    PreRotateY = 23,
    ZScale = 24,
    ZTranslate = 25,
    ZCone = 26,
    PostRotateX = 27,
    PostRotateY = 28,
}

/// Names as they appear in `.flame` XML attributes, indexed by `Builtin`.
pub const BUILTIN_NAMES: [&str; 29] = [
    "linear",
    "flatten",
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
    "noise",
    "blur",
    "gaussian_blur",
    "zblur",
    "blur3D",
    "pre_blur",
    "pre_zscale",
    "pre_ztranslate",
    "pre_rotate_x",
    "pre_rotate_y",
    "zscale",
    "ztranslate",
    "zcone",
    "post_rotate_x",
    "post_rotate_y",
];

impl Builtin {
    pub fn from_name(name: &str) -> Option<Builtin> {
        BUILTIN_NAMES.iter().position(|n| *n == name).map(|i| Builtin::from_index(i))
    }

    pub fn from_index(i: usize) -> Builtin {
        assert!(i < 29, "builtin index out of range: {i}");
        // Safe: repr(u8), contiguous 0..29, bounds checked above.
        unsafe { core::mem::transmute(i as u8) }
    }

    pub fn name(self) -> &'static str {
        BUILTIN_NAMES[self as usize]
    }

    fn pass(self) -> Pass {
        use Builtin::*;
        match self {
            PreBlur | PreZScale | PreZTranslate | PreRotateX | PreRotateY => Pass::Pre,
            // `flatten` has no `post_` prefix but runs in the post pass.
            Flatten | PostRotateX | PostRotateY => Pass::Post,
            _ => Pass::Normal,
        }
    }

    fn precalc(self) -> Precalc {
        use Builtin::*;
        match self {
            Polar | Disc => Precalc::Angle,
            Spiral | Diamond => Precalc::SinCos,
            _ => Precalc::None,
        }
    }
}

/// A configured builtin: kind, weight, and up to two precomputed constants.
#[derive(Clone)]
pub struct BuiltinVar {
    kind: Builtin,
    /// `vvar` in the original — this variation's weight for the owning xform.
    w: f64,
    /// Precomputed constants. Meaning depends on `kind`:
    ///   Polar/Disc          -> k1 = w/pi
    ///   Pre/PostRotateX/Y   -> k1 = sin(w*pi/2), k2 = cos(w*pi/2)
    k1: f64,
    k2: f64,
}

impl BuiltinVar {
    pub fn new(kind: Builtin) -> Self {
        BuiltinVar { kind, w: 0.0, k1: 0.0, k2: 0.0 }
    }

    pub fn kind(&self) -> Builtin {
        self.kind
    }
}

impl Variation for BuiltinVar {
    fn name(&self) -> &'static str {
        self.kind.name()
    }

    fn pass(&self) -> Pass {
        self.kind.pass()
    }

    fn precalc(&self) -> Precalc {
        self.kind.precalc()
    }

    fn prepare(&mut self, weight: f64, _coefs: &Affine, _rng: &mut Rng) {
        use Builtin::*;
        self.w = weight;
        match self.kind {
            Polar | Disc => self.k1 = weight / core::f64::consts::PI,
            PreRotateX | PreRotateY | PostRotateX | PostRotateY => {
                let a = weight * core::f64::consts::FRAC_PI_2;
                self.k1 = a.sin();
                self.k2 = a.cos();
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn calc(&self, st: &mut VarState, rng: &mut Rng, g: &mut GaussBuf) {
        use Builtin::*;
        let v = self.w;
        let (tx, ty, tz) = (st.tx, st.ty, st.tz);

        match self.kind {
            Linear => {
                st.px += v * tx;
                st.py += v * ty;
                st.pz += v * tz;
            }
            // Takes no weight — it simply zeroes z in the post pass.
            Flatten => {
                st.pz = 0.0;
            }
            Sinusoidal => {
                st.px += v * tx.sin();
                st.py += v * ty.sin();
                st.pz += v * tz;
            }
            Spherical => {
                let r = v / (tx * tx + ty * ty + EPS);
                st.px += tx * r;
                st.py += ty * r;
                st.pz += v * tz;
            }
            Swirl => {
                let r = tx * tx + ty * ty;
                let (s, c) = r.sin_cos();
                st.px += v * (s * tx - c * ty);
                st.py += v * (c * tx + s * ty);
                st.pz += v * tz;
            }
            Horseshoe => {
                let r = v / ((tx * tx + ty * ty).sqrt() + EPS);
                st.px += (tx - ty) * (tx + ty) * r;
                st.py += 2.0 * tx * ty * r;
                st.pz += v * tz;
            }
            Polar => {
                st.px += self.k1 * st.angle;
                st.py += v * ((tx * tx + ty * ty).sqrt() - 1.0);
                st.pz += v * tz;
            }
            Disc => {
                let (s, c) = (core::f64::consts::PI * (tx * tx + ty * ty).sqrt()).sin_cos();
                let r = self.k1 * st.angle;
                st.px += s * r;
                st.py += c * r;
                st.pz += v * tz;
            }
            Spiral => {
                let r = st.length + 1e-6;
                let (s, c) = r.sin_cos();
                let r = v / r;
                st.px += (st.cos_a + s) * r;
                st.py += (st.sin_a - c) * r;
                st.pz += v * tz;
            }
            // NOTE: this is Apophysis's hyperbolic, not flam3's. flam3 uses
            // `v*sin(theta)/r` and `v*r*cos(theta)`; this one is a plain
            // x-inversion with y passed through. Do not "correct" it.
            Hyperbolic => {
                st.px += v * tx / (tx * tx + ty * ty + EPS);
                st.py += v * ty;
                st.pz += v * tz;
            }
            // Named `diamond` in the tables; the Delphi method is `Square`.
            Diamond => {
                let (s, c) = st.length.sin_cos();
                st.px += v * st.sin_a * c;
                st.py += v * st.cos_a * s;
                st.pz += v * tz;
            }
            Eyefish => {
                let r = 2.0 * v / ((tx * tx + ty * ty).sqrt() + 1.0);
                st.px += r * tx;
                st.py += r * ty;
                st.pz += v * tz;
            }
            Bubble => {
                let r = (tx * tx + ty * ty) / 4.0 + 1.0;
                st.pz += v * (2.0 / r - 1.0);
                let r = v / r;
                st.px += r * tx;
                st.py += r * ty;
            }
            Cylinder => {
                st.px += v * tx.sin();
                st.py += v * ty;
                st.pz += v * tx.cos();
            }
            Noise => {
                let (s, c) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                let r = v * rng.f64();
                st.px += tx * r * c;
                st.py += ty * r * s;
                st.pz += v * tz;
            }
            Blur => {
                let (s, c) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                let r = v * rng.f64();
                st.px += r * c;
                st.py += r * s;
                st.pz += v * tz;
            }
            GaussianBlur => {
                let r = v * g.next(rng);
                let (s, c) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                st.px += r * c;
                st.py += r * s;
                st.pz += v * tz;
            }
            ZBlur => {
                st.pz += v * g.next(rng);
            }
            Blur3D => {
                let r = v * g.next(rng);
                let (sa, ca) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                let (sb, cb) = (rng.f64() * core::f64::consts::PI).sin_cos();
                st.px += r * sb * ca;
                st.py += r * sb * sa;
                st.pz += r * cb;
            }
            PreBlur => {
                let r = v * g.next(rng);
                let (s, c) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                st.tx += r * c;
                st.ty += r * s;
            }
            PreZScale => {
                st.tz = tz * v;
            }
            PreZTranslate => {
                st.tz = tz + v;
            }
            PreRotateX => {
                let z = self.k2 * tz - self.k1 * ty;
                st.ty = self.k1 * tz + self.k2 * ty;
                st.tz = z;
            }
            PreRotateY => {
                let x = self.k2 * tx - self.k1 * tz;
                st.tz = self.k1 * tx + self.k2 * tz;
                st.tx = x;
            }
            ZScale => {
                st.pz += v * tz;
            }
            ZTranslate => {
                st.pz += v;
            }
            ZCone => {
                st.pz += v * (tx * tx + ty * ty).sqrt();
            }
            PostRotateX => {
                let z = self.k2 * st.pz - self.k1 * st.py;
                st.py = self.k1 * st.pz + self.k2 * st.py;
                st.pz = z;
            }
            PostRotateY => {
                let x = self.k2 * st.px - self.k1 * st.pz;
                st.pz = self.k1 * st.px + self.k2 * st.pz;
                st.px = x;
            }
        }
    }
}
