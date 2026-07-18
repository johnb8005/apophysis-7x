// Apophysis 7X Copyright (C) 2009-2010 Georg Kiehne
// Apophysis Copyright (C) 2001-2004 Mark Townsend
// Apophysis Copyright (C) 2005-2006 Ronald Hordijk, Piotr Borys, Peter Sdobnov
// Apophysis Copyright (C) 2007-2008 Piotr Borys, Peter Sdobnov
// Apophysis "3D hack" Copyright (C) 2007-2008 Peter Sdobnov
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version. See LICENSE at the repository root.

//! Deterministic PRNG for the chaos game.
//!
//! The Delphi original used the global `Random` plus, in several blur
//! variations, a per-point call to `Randomize` — which reseeds from the system
//! clock on *every iteration*. The source flags this itself:
//! `// Randomize here = HACK! Fix me...` (XForm.pas). Reproducing that would
//! make renders non-reproducible and is a large part of why the original's
//! blur variations are slow, so we deliberately do not port it: each worker
//! owns one seeded stream instead.

/// xoshiro256++ — fast, well-distributed, and trivially splittable per worker.
#[derive(Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 to spread a single seed over the full state.
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Rng { s: [next(), next(), next(), next()] }
    }

    #[inline(always)]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1) — matches Delphi's `Random: Extended` contract.
    #[inline(always)]
    pub fn f64(&mut self) -> f64 {
        // Top 53 bits give a uniform double without rejection.
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [-1, 1).
    #[inline(always)]
    pub fn f64_signed(&mut self) -> f64 {
        2.0 * self.f64() - 1.0
    }

    /// Uniform integer in [0, n) — matches Delphi's `Random(n)`.
    #[inline(always)]
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        // Multiply-shift; bias is negligible at the ranges we use (n <= 1024).
        ((self.next_u64() >> 32) as u32 as u64 * n as u64 >> 32) as u32
    }
}

/// The 4-tap rolling uniform sum used by `gaussian_blur`, `zblur`, `blur3D`
/// and `pre_blur`.
///
/// The original keeps four uniforms per xform and replaces one per call
/// round-robin, yielding `g0+g1+g2+g3 - 2` — an Irwin-Hall approximation to a
/// Gaussian. Successive draws share three of four taps, so the sequence is
/// autocorrelated; that correlation is visually load-bearing (it is what the
/// original's blur actually looks like), so we reproduce it exactly rather
/// than substituting a true Gaussian.
#[derive(Clone)]
pub struct GaussBuf {
    taps: [f64; 4],
    n: usize,
}

impl GaussBuf {
    pub fn new(rng: &mut Rng) -> Self {
        GaussBuf { taps: [rng.f64(), rng.f64(), rng.f64(), rng.f64()], n: 0 }
    }

    #[inline(always)]
    pub fn next(&mut self, rng: &mut Rng) -> f64 {
        let sum = self.taps[0] + self.taps[1] + self.taps[2] + self.taps[3] - 2.0;
        self.taps[self.n] = rng.f64();
        self.n = (self.n + 1) & 3;
        sum
    }
}
