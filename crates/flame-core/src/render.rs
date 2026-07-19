// Ported from Apophysis 7X `src/Rendering/RenderingInterface.pas` (camera and
// buffer sizing), `RenderingImplementation.pas` (batch loop) and
// `ImageMaker.pas` (filter + tone mapping).
// See LICENSE (GPL-2.0-or-later) at the repo root.

use crate::genome::{
    Buckets, Camera, Flame, BRIGHT_ADJUST, FILTER_CUTOFF, MAX_FILTER_WIDTH, PREFILTER_WHITE,
    SUB_BATCH_SIZE,
};
use crate::rng::Rng;

/// Normalized 2D Gaussian reconstruction filter (`TImageMaker.CreateFilter`).
pub struct Filter {
    pub size: usize,
    pub kernel: Vec<f64>,
}

impl Filter {
    pub fn new(oversample: usize, radius: f64) -> Filter {
        let fw = (2.0 * FILTER_CUTOFF * oversample as f64 * radius).trunc();
        let mut size = fw as usize + 1;
        // The filter must have the same parity as the oversample factor, so
        // that the kernel centre lands on a bucket centre.
        if (size + oversample) % 2 == 1 {
            size += 1;
        }
        let adjust = if fw > 0.0 { FILTER_CUTOFF * size as f64 / fw } else { 1.0 };

        let mut kernel = vec![0.0; size * size];
        for i in 0..size {
            for j in 0..size {
                let ii = ((2.0 * i as f64 + 1.0) / size as f64 - 1.0) * adjust;
                let jj = ((2.0 * j as f64 + 1.0) / size as f64 - 1.0) * adjust;
                kernel[i * size + j] = (-2.0 * (ii * ii + jj * jj)).exp();
            }
        }
        let sum: f64 = kernel.iter().sum();
        if sum > 0.0 {
            for k in kernel.iter_mut() {
                *k /= sum;
            }
        }
        Filter { size, kernel }
    }
}

/// Buffer geometry and the world-to-bucket camera.
pub struct RenderGeometry {
    pub bucket_width: usize,
    pub bucket_height: usize,
    pub camera: Camera,
    pub gutter_width: usize,
}

impl RenderGeometry {
    pub fn new(flame: &Flame, filter_size: usize) -> RenderGeometry {
        let oversample = flame.spatial_oversample.max(1);

        // The buffer is always padded for the widest supported filter, but the
        // camera is offset by the *actual* filter's gutter — which is why the
        // tone-mapping pass can start its window at bucket 0.
        let max_gutter = (MAX_FILTER_WIDTH - oversample) / 2;
        let gutter = filter_size.saturating_sub(oversample) / 2;

        let bucket_width = oversample * flame.width + 2 * max_gutter;
        let bucket_height = oversample * flame.height + 2 * max_gutter;

        let ppu = flame.ppux();
        let corner_x = flame.center[0] - flame.width as f64 / ppu / 2.0;
        let corner_y = flame.center[1] - flame.height as f64 / ppu / 2.0;

        let os = oversample as f64;
        let t0 = gutter as f64 / (os * ppu);
        let t2 = (2 * max_gutter - gutter) as f64 / (os * ppu);

        let cam_x0 = corner_x - t0;
        let cam_x1 = corner_x + flame.width as f64 / ppu + t2;
        let cam_y0 = corner_y - t0;
        let cam_y1 = corner_y + flame.height as f64 / ppu + t2;

        let mut cam_w = cam_x1 - cam_x0;
        let mut cam_h = cam_y1 - cam_y0;
        // The original guards a degenerate camera by falling back to 1.
        if cam_w.abs() <= 0.01 {
            cam_w = 1.0;
        }
        if cam_h.abs() <= 0.01 {
            cam_h = 1.0;
        }

        let bws = (bucket_width as f64 - 0.5) / cam_w;
        let bhs = (bucket_height as f64 - 0.5) / cam_h;

        let rotate = flame.angle != 0.0;
        let (sina, cosa) = if rotate { flame.angle.sin_cos() } else { (0.0, 1.0) };
        let rcx = flame.center[0] * (1.0 - cosa) - flame.center[1] * sina - cam_x0;
        let rcy = flame.center[1] * (1.0 - cosa) + flame.center[0] * sina - cam_y0;

        RenderGeometry {
            bucket_width,
            bucket_height,
            gutter_width: gutter,
            camera: Camera { cam_x0, cam_y0, cam_w, cam_h, bws, bhs, rotate, cosa, sina, rcx, rcy },
        }
    }
}

/// An RGBA8 image.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// Render a prepared flame to an RGBA8 image.
///
/// `flame.prepare()` must have been called first.
pub fn render(flame: &Flame, seed: u64) -> Image {
    let filter = Filter::new(flame.spatial_oversample.max(1), flame.spatial_filter_radius);
    let geom = RenderGeometry::new(flame, filter.size);
    let mut buckets =
        Buckets::new(geom.bucket_width, geom.bucket_height, &flame.palette, flame.white_level);

    // Total sample count, scaled by zoom exactly as CreateCamera does.
    let scale = 2f64.powf(flame.zoom);
    let sample_density = flame.sample_density * scale * scale;
    let bucket_size = (geom.bucket_width * geom.bucket_height) as f64;
    let oversample = flame.spatial_oversample.max(1) as f64;
    let nsamples = sample_density * bucket_size / (oversample * oversample);
    let batches = ((nsamples / SUB_BATCH_SIZE as f64).round() as usize).max(1);

    let mut rng = Rng::new(seed);
    for _ in 0..batches {
        flame.iterate_batch(&mut buckets, &geom.camera, &mut rng);
    }

    // `actual_density` is what was really sampled, which is what the tone
    // mapper must use — otherwise a truncated render comes out too dark.
    // Because `batches` was derived from the zoom-scaled sample_density, this
    // already carries the `scale²` factor that Delphi's CreateImage applies to
    // its nominal `fcp.actual_density` (ImageMaker.pas:448) — the tone mapper
    // must not apply it again.
    let actual_density =
        batches as f64 * SUB_BATCH_SIZE as f64 * oversample * oversample / bucket_size;

    tone_map(flame, &buckets, &filter, &geom, actual_density)
}

/// Convert the accumulation buffer to pixels (`TImageMaker.CreateImage`).
pub fn tone_map(
    flame: &Flame,
    buckets: &Buckets,
    filter: &Filter,
    _geom: &RenderGeometry,
    actual_density: f64,
) -> Image {
    let oversample = flame.spatial_oversample.max(1);
    let gamma = if flame.gamma == 0.0 { flame.gamma } else { 1.0 / flame.gamma };
    let vib = (flame.vibrancy * 256.0).round();
    let notvib = 256.0 - vib;

    let funcval =
        if flame.gamma_threshold != 0.0 { flame.gamma_threshold.powf(gamma - 1.0) } else { 0.0 };

    // `actual_density` is the sampled density and already includes the zoom
    // factor (see `render`), so it is used as-is. Delphi reaches the same
    // value by scaling a nominal density once here (ImageMaker.pas:448).
    let mut sample_density = actual_density;
    if sample_density == 0.0 {
        sample_density = 0.001;
    }

    // `brightness` is stored exactly as the XML carries it — the XML reader
    // (Main.pas:5109) does no scaling. Only the legacy *text* parser divides
    // by BRIGHT_ADJUST (ControlPoint.pas:932), which is why that factor
    // reappears here rather than at load time.
    let k1 = flame.contrast * BRIGHT_ADJUST * flame.brightness * 268.0 * PREFILTER_WHITE / 256.0;
    let ppu = flame.ppux();
    let area = flame.width as f64 * flame.height as f64 / (ppu * ppu);
    let k2 = (oversample * oversample) as f64
        / (flame.contrast * area * flame.white_level * sample_density);

    let white = flame.white_level;
    // Log-density scale factor for a given hit count.
    let ls_for = |count: f64| -> f64 {
        if count <= 0.0 {
            0.0
        } else {
            k1 * (1.0 + white * count * k2).log10() / (white * count)
        }
    };
    // The original precomputes counts 0..1024 into a lookup table; at f64 the
    // table and the closure agree exactly, so we skip it.

    // Built once per image; skipped entirely when every curve is the identity.
    let lut = flame.curves.build_lut();

    let bgi = [
        flame.background[0].round(),
        flame.background[1].round(),
        flame.background[2].round(),
    ];

    let mut out = vec![0u8; flame.width * flame.height * 4];

    for y in 0..flame.height {
        for x in 0..flame.width {
            let bx = x * oversample;
            let by = y * oversample;

            let mut fp = [0.0f64; 4];
            if filter.size > 1 {
                for ii in 0..filter.size {
                    for jj in 0..filter.size {
                        let fv = filter.kernel[ii * filter.size + jj];
                        let (sx, sy) = (bx + jj, by + ii);
                        if sx >= buckets.width || sy >= buckets.height {
                            continue;
                        }
                        let b = &buckets.data[sy * buckets.width + sx];
                        let ls = ls_for(b[3]);
                        fp[0] += fv * ls * b[0];
                        fp[1] += fv * ls * b[1];
                        fp[2] += fv * ls * b[2];
                        fp[3] += fv * ls * b[3];
                    }
                }
            } else {
                let b = &buckets.data[by * buckets.width + bx];
                let ls = ls_for(b[3]);
                fp = [ls * b[0], ls * b[1], ls * b[2], ls * b[3]];
            }
            fp[0] /= PREFILTER_WHITE;
            fp[1] /= PREFILTER_WHITE;
            fp[2] /= PREFILTER_WHITE;
            fp[3] = white * fp[3] / PREFILTER_WHITE;

            let o = (y * flame.width + x) * 4;

            if fp[3] <= 0.0 {
                // Empty pixel: solid background (or fully transparent).
                if flame.transparent {
                    out[o..o + 4].copy_from_slice(&[0, 0, 0, 0]);
                } else {
                    out[o] = bgi[0] as u8;
                    out[o + 1] = bgi[1] as u8;
                    out[o + 2] = bgi[2] as u8;
                    out[o + 3] = 255;
                }
                continue;
            }

            // Gamma, with a linear ramp below the threshold to avoid crushing
            // sparse pixels into noise.
            let alpha = if flame.gamma_threshold != 0.0 && fp[3] <= flame.gamma_threshold {
                let frac = fp[3] / flame.gamma_threshold;
                (1.0 - frac) * fp[3] * funcval + frac * fp[3].powf(gamma)
            } else {
                fp[3].powf(gamma)
            };

            let ls = vib * alpha / fp[3];
            let ai = (alpha * 256.0).round().clamp(0.0, 255.0);
            let ia = 255.0 - ai;

            // Vibrancy blends per-channel gamma against luminance-scaled gamma.
            let mut rgb = if notvib > 0.0 {
                [
                    ls * fp[0] + notvib * fp[0].max(0.0).powf(gamma),
                    ls * fp[1] + notvib * fp[1].max(0.0).powf(gamma),
                    ls * fp[2] + notvib * fp[2].max(0.0).powf(gamma),
                ]
            } else {
                [ls * fp[0], ls * fp[1], ls * fp[2]]
            };

            // Curves run before the background composite, matching the
            // original's ordering in CreateImage.
            if lut.active() {
                for c in 0..3 {
                    rgb[c] = lut.apply(c, rgb[c].round());
                }
            }

            if flame.transparent {
                // Un-premultiply rather than compositing over the background.
                if ai <= 0.0 {
                    out[o..o + 4].copy_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                for c in 0..3 {
                    rgb[c] = (rgb[c].round() * 255.0 / ai).clamp(0.0, 255.0);
                    out[o + c] = rgb[c] as u8;
                }
                out[o + 3] = ai as u8;
            } else {
                for c in 0..3 {
                    // Integer shift in the original, so background weight is
                    // (255-ai)/256 rather than /255.
                    let v = rgb[c].round() + (ia * bgi[c]).floor() / 256.0;
                    out[o + c] = v.clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = 255;
            }
        }
    }

    Image { width: flame.width, height: flame.height, data: out }
}

#[cfg(test)]
mod tests {
    use crate::builtins::{Builtin, BuiltinVar};
    use crate::flame::{Affine, XForm};
    use crate::genome::Flame;
    use crate::rng::Rng;
    use crate::variation::Variation;

    fn tiny_flame() -> Flame {
        let mut f = Flame::default();
        let half = |e: f64, ff: f64| Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e, f: ff };
        for (coefs, color) in [(half(0.0, 0.0), 0.0), (half(0.5, 0.0), 0.5), (half(0.25, 0.5), 1.0)]
        {
            let mut xf = XForm::default();
            xf.coefs = coefs;
            xf.color = color;
            xf.density = 1.0;
            xf.set_variations(vec![(
                Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
                1.0,
            )]);
            f.xforms.push(xf);
        }
        f.width = 96;
        f.height = 96;
        f.center = [0.5, 0.5];
        f.pixels_per_unit = 96.0;
        f.sample_density = 40.0;
        f
    }

    fn mean_luma(f: &Flame) -> f64 {
        let mut f = f.clone();
        let mut rng = Rng::new(1);
        f.prepare(&mut rng);
        let img = super::render(&f, 1);
        let sum: u64 = img
            .data
            .chunks(4)
            .map(|p| (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3)
            .sum();
        sum as f64 / (img.width * img.height) as f64
    }

    /// zoom=z with pixels_per_unit divided by 2^z frames the identical image
    /// (ppux = ppu * 2^zoom), just sampled 4^z times as densely. The tone
    /// mapper must keep brightness invariant across that trade — the original
    /// applies the zoom quality factor exactly once (ImageMaker.pas:448).
    /// Applying it twice made every zoomed render come out dark.
    #[test]
    fn zoom_quality_factor_applied_exactly_once() {
        let base = tiny_flame();
        let plain = mean_luma(&base);

        let mut zoomed = base.clone();
        zoomed.zoom = 1.0;
        zoomed.pixels_per_unit = base.pixels_per_unit / 2.0;
        let z = mean_luma(&zoomed);

        assert!(plain > 5.0, "test flame renders black: {plain}");
        assert!(
            (z - plain).abs() / plain < 0.1,
            "zoom changed brightness: zoom=0 -> {plain}, zoom=1 -> {z}"
        );
    }
}
