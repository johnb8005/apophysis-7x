// Ported from Apophysis 7X `src/Flame/ControlPoint.pas`.
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::flame::{Point, XForm};
use crate::rng::{GaussBuf, Rng};

/// Constants from `ControlPoint.pas:35-40`.
pub const SUB_BATCH_SIZE: usize = 10_000;
pub const PROP_TABLE_SIZE: usize = 1024;
pub const PREFILTER_WHITE: f64 = (1u64 << 26) as f64;
pub const FILTER_CUTOFF: f64 = 1.8;
pub const BRIGHT_ADJUST: f64 = 2.3;
/// `for i := 0 to FUSE` is inclusive, so this yields FUSE+1 = 16 warmup
/// iterations. Naming it FUSE and adding 1 at the use site keeps the
/// correspondence to the original obvious.
pub const FUSE: usize = 15;

/// 256-entry RGB palette.
#[derive(Clone)]
pub struct Palette(pub Vec<[u8; 3]>);

impl Default for Palette {
    fn default() -> Self {
        // A neutral ramp, so a genome without a palette still renders.
        Palette((0..256).map(|i| [i as u8, i as u8, i as u8]).collect())
    }
}

impl Palette {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Camera projection mode, chosen by `Prepare` from which 3D fields are set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Projection {
    None,
    Pitch,
    PitchYaw,
    Dof,
}

/// A complete flame document — the port of `TControlPoint`.
#[derive(Clone)]
pub struct Flame {
    pub xforms: Vec<XForm>,
    /// Optional final xform, applied to each plotted point without advancing
    /// the orbit.
    pub final_xform: Option<XForm>,
    pub final_enabled: bool,

    // Camera
    pub center: [f64; 2],
    /// XML `scale`.
    pub pixels_per_unit: f64,
    pub zoom: f64,
    /// Image rotation, radians (XML `angle`).
    pub angle: f64,
    pub width: usize,
    pub height: usize,
    pub cam_pitch: f64,
    pub cam_yaw: f64,
    pub cam_persp: f64,
    pub cam_zpos: f64,
    pub cam_dof: f64,

    // Tone
    pub brightness: f64,
    pub contrast: f64,
    pub gamma: f64,
    pub vibrancy: f64,
    pub gamma_threshold: f64,
    pub background: [f64; 3],
    pub white_level: f64,
    pub transparent: bool,

    // Sampling
    /// XML `quality`.
    pub sample_density: f64,
    pub spatial_oversample: usize,
    pub spatial_filter_radius: f64,

    pub palette: Palette,
    /// Bezier tone curves: overall, red, green, blue.
    pub curves: crate::curves::Curves,
    pub name: String,

    /// -1 = none. When set, every other xform's opacity is forced to 0.
    pub solo_xform: i32,

    projection: Projection,
    dof_coef: f64,
    camera_matrix: [[f64; 3]; 3],
    /// Per-source-xform 1024-entry selection tables (the xaos Markov chain).
    prop_tables: Vec<Vec<usize>>,
}

impl Default for Flame {
    fn default() -> Self {
        Flame {
            xforms: Vec::new(),
            final_xform: None,
            final_enabled: false,
            center: [0.0, 0.0],
            pixels_per_unit: 25.0,
            zoom: 0.0,
            angle: 0.0,
            width: 640,
            height: 480,
            cam_pitch: 0.0,
            cam_yaw: 0.0,
            cam_persp: 0.0,
            cam_zpos: 0.0,
            cam_dof: 0.0,
            brightness: 4.0,
            contrast: 1.0,
            gamma: 4.0,
            vibrancy: 1.0,
            gamma_threshold: 0.0,
            background: [0.0, 0.0, 0.0],
            white_level: 200.0,
            transparent: false,
            sample_density: 50.0,
            spatial_oversample: 1,
            spatial_filter_radius: 0.5,
            palette: Palette::default(),
            curves: crate::curves::Curves::default(),
            name: String::new(),
            solo_xform: -1,
            projection: Projection::None,
            dof_coef: 0.0,
            camera_matrix: [[0.0; 3]; 3],
            prop_tables: Vec::new(),
        }
    }
}

impl Flame {
    /// Effective pixels-per-unit after zoom (`getppux`).
    pub fn ppux(&self) -> f64 {
        self.pixels_per_unit * 2f64.powf(self.zoom)
    }

    /// Precompute everything the render loop needs: xform prep, the xaos
    /// selection tables, and the camera matrix.
    pub fn prepare(&mut self, rng: &mut Rng) {
        // Solo forces all other opacities to zero (ControlPoint.pas:427).
        if self.solo_xform >= 0 {
            for (i, xf) in self.xforms.iter_mut().enumerate() {
                xf.opacity = if i as i32 == self.solo_xform { 1.0 } else { 0.0 };
            }
        }

        for xf in self.xforms.iter_mut() {
            xf.prepare(rng);
        }
        if let Some(fx) = self.final_xform.as_mut() {
            fx.prepare(rng);
        }

        self.build_prop_tables();
        self.build_camera();
    }

    /// Build one 1024-entry table per source xform.
    ///
    /// Transition weight from xform `k` to `i` is
    /// `density[i] * modWeights[k][i]`, discretized into 1024 slots. Because
    /// the table is per-source, transitions form a first-order Markov chain
    /// rather than independent draws — this is what makes xaos work.
    fn build_prop_tables(&mut self) {
        let n = self.xforms.len();
        let mut tables = Vec::with_capacity(n);

        for k in 0..n {
            let tp: Vec<f64> = (0..n)
                .map(|i| {
                    let mw = self.xforms[k].mod_weights.get(i).copied().unwrap_or(1.0);
                    self.xforms[i].density * mw
                })
                .collect();
            let total: f64 = tp.iter().sum();

            let mut table = vec![usize::MAX; PROP_TABLE_SIZE];
            if total > 0.0 {
                let mut loop_value = 0.0;
                for slot in table.iter_mut() {
                    let mut propsum = 0.0;
                    let mut j = 0usize;
                    loop {
                        propsum += tp[j];
                        if propsum > loop_value || j == n - 1 {
                            break;
                        }
                        j += 1;
                    }
                    *slot = j;
                    loop_value += total / PROP_TABLE_SIZE as f64;
                }
            }
            // A zero-sum row leaves usize::MAX sentinels; the original stores
            // `invalidXform`, which raises EMathError on use. We check instead.
            tables.push(table);
        }
        self.prop_tables = tables;
    }

    /// Camera matrix, indexed `[column][row]` as in the original.
    fn build_camera(&mut self) {
        let (sp, cp) = self.cam_pitch.sin_cos();
        let (sy, cy) = (-self.cam_yaw).sin_cos();
        self.camera_matrix = [
            [cy, cp * sy, sp * sy],
            [-sy, cp * cy, sp * cy],
            [0.0, -sp, cp],
        ];
        self.dof_coef = 0.1 * self.cam_dof;

        // DOF takes priority over the pitch-only path.
        self.projection = if self.cam_dof != 0.0 {
            Projection::Dof
        } else if self.cam_yaw != 0.0 {
            Projection::PitchYaw
        } else if self.cam_pitch != 0.0 {
            Projection::Pitch
        } else {
            Projection::None
        };
    }

    /// Project a world point to camera space.
    #[inline(always)]
    fn project(&self, p: &mut Point, rng: &mut Rng) {
        let m = &self.camera_matrix;
        match self.projection {
            Projection::None => {
                let zr = 1.0 - self.cam_persp * (p.z - self.cam_zpos);
                p.x /= zr;
                p.y /= zr;
                p.z -= self.cam_zpos;
            }
            Projection::Pitch => {
                let z = p.z - self.cam_zpos;
                let y = m[1][1] * p.y + m[2][1] * z;
                let zr = 1.0 - self.cam_persp * (m[1][2] * p.y + m[2][2] * z);
                p.y = y / zr;
                p.x /= zr;
                p.z = z;
            }
            Projection::PitchYaw => {
                let z = p.z - self.cam_zpos;
                let x = m[0][0] * p.x + m[1][0] * p.y;
                let y = m[0][1] * p.x + m[1][1] * p.y + m[2][1] * z;
                let zr =
                    1.0 - self.cam_persp * (m[0][2] * p.x + m[1][2] * p.y + m[2][2] * z);
                p.x = x / zr;
                p.y = y / zr;
                p.z = z;
            }
            Projection::Dof => {
                let z0 = p.z - self.cam_zpos;
                let x = m[0][0] * p.x + m[1][0] * p.y;
                let y = m[0][1] * p.x + m[1][1] * p.y + m[2][1] * z0;
                let z = m[0][2] * p.x + m[1][2] * p.y + m[2][2] * z0;
                let zr = 1.0 - self.cam_persp * z;
                let (ds, dc) = (rng.f64() * core::f64::consts::TAU).sin_cos();
                // Uniform in radius (denser toward the disc centre), and
                // deliberately unclamped for negative z — points behind the
                // focal plane get a 180-degree-flipped offset, as in the
                // original.
                let dr = rng.f64() * self.dof_coef * z;
                p.x = (x + dr * dc) / zr;
                p.y = (y + dr * ds) / zr;
                p.z = z0;
            }
        }
    }

    /// Run one sub-batch of the chaos game, accumulating into `buckets`.
    ///
    /// Each batch reseeds the orbit from a fresh random point, so batches are
    /// independent and can be distributed across workers freely.
    pub fn iterate_batch(&self, buckets: &mut Buckets, cam: &Camera, rng: &mut Rng) {
        if self.xforms.is_empty() {
            return;
        }
        let mut g = GaussBuf::new(rng);

        let mut p = Point {
            x: rng.f64_signed(),
            y: rng.f64_signed(),
            z: 0.0,
            c: rng.f64(),
            o: 1.0,
        };

        let mut xi = 0usize;
        // FUSE+1 warmup iterations, discarded.
        for _ in 0..=FUSE {
            match self.next_xform(xi, rng) {
                Some(next) => xi = next,
                None => return,
            }
            self.xforms[xi].next_point(&mut p, rng, &mut g);
        }

        let use_final = self.final_enabled && self.final_xform.is_some();
        let mut out = Point::default();

        for _ in 0..SUB_BATCH_SIZE {
            match self.next_xform(xi, rng) {
                Some(next) => xi = next,
                None => return,
            }
            let xf = &self.xforms[xi];
            xf.next_point(&mut p, rng, &mut g);

            // Stochastic opacity reject.
            if rng.f64() >= xf.opacity {
                continue;
            }

            // The final xform sees the point but must not advance the orbit.
            // It also re-blends colour, so the palette index comes from its
            // output rather than from `p`.
            let (mut q, cidx) = if use_final {
                let fx = self.final_xform.as_ref().unwrap();
                fx.next_point_to(&p, &mut out, rng, &mut g);
                (out, out.c)
            } else {
                (p, p.c)
            };

            self.project(&mut q, rng);
            buckets.plot(cam, q.x, q.y, cidx);
        }
    }

    #[inline(always)]
    fn next_xform(&self, from: usize, rng: &mut Rng) -> Option<usize> {
        let table = self.prop_tables.get(from)?;
        let next = table[rng.below(PROP_TABLE_SIZE as u32) as usize];
        if next == usize::MAX {
            None
        } else {
            Some(next)
        }
    }
}

/// World-to-bucket mapping, the port of `TBaseRenderer.CreateCamera`.
#[derive(Clone, Copy)]
pub struct Camera {
    pub cam_x0: f64,
    pub cam_y0: f64,
    pub cam_w: f64,
    pub cam_h: f64,
    /// Bucket scale factors.
    pub bws: f64,
    pub bhs: f64,
    /// Rotation, precomputed only when angle != 0.
    pub rotate: bool,
    pub cosa: f64,
    pub sina: f64,
    pub rcx: f64,
    pub rcy: f64,
}

/// The widest filter the buffer is padded for (`MAX_FILTER_WIDTH`).
pub const MAX_FILTER_WIDTH: usize = 25;

/// Accumulation buffer: R, G, B, count per bucket.
///
/// The 32-bit Delphi build uses f32 here and the 64-bit build f64; we use f64
/// throughout, which diverges from the 32-bit build on very long renders of
/// bright pixels. That is a deliberate accuracy improvement, not an oversight.
pub struct Buckets {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f64; 4]>,
    /// Palette premultiplied by `white_level / 256`, as `CreateColorMap` does.
    pub colormap: Vec<[f64; 3]>,
}

impl Buckets {
    pub fn new(width: usize, height: usize, palette: &Palette, white_level: f64) -> Self {
        // NOTE: integer `div 256` in the original — truncating, not rounding.
        let colormap = palette
            .0
            .iter()
            .map(|c| {
                [
                    ((c[0] as i64 * white_level as i64) / 256) as f64,
                    ((c[1] as i64 * white_level as i64) / 256) as f64,
                    ((c[2] as i64 * white_level as i64) / 256) as f64,
                ]
            })
            .collect();
        Buckets { width, height, data: vec![[0.0; 4]; width * height], colormap }
    }

    #[inline(always)]
    pub fn plot(&mut self, cam: &Camera, x: f64, y: f64, c: f64) {
        let (px, py) = if cam.rotate {
            (x * cam.cosa + y * cam.sina + cam.rcx, y * cam.cosa - x * cam.sina + cam.rcy)
        } else {
            (x - cam.cam_x0, y - cam.cam_y0)
        };
        if px < 0.0 || px > cam.cam_w || py < 0.0 || py > cam.cam_h {
            return;
        }

        let bx = (cam.bws * px).round() as usize;
        let by = (cam.bhs * py).round() as usize;
        if bx >= self.width || by >= self.height {
            return;
        }

        // Palette index is unclamped in the original; NaN would panic on cast,
        // so we clamp defensively.
        let ci = ((c * 255.0).round().clamp(0.0, 255.0)) as usize;
        let col = self.colormap[ci.min(self.colormap.len() - 1)];

        let b = &mut self.data[by * self.width + bx];
        b[0] += col[0];
        b[1] += col[1];
        b[2] += col[2];
        b[3] += 1.0;
    }

    pub fn merge(&mut self, other: &Buckets) {
        for (a, b) in self.data.iter_mut().zip(other.data.iter()) {
            a[0] += b[0];
            a[1] += b[1];
            a[2] += b[2];
            a[3] += b[3];
        }
    }
}
