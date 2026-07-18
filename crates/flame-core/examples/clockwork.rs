//! An attempt at the "clockwork" style: dense monochrome gear rings on a light
//! ground, in the manner of fractal.jpg.
//!
//! The look comes from four things:
//!   * `julian` at a high power gives N-fold rotational symmetry — the gear
//!     teeth and the radiating spokes
//!   * `rings2` lays down concentric bands
//!   * a contracting rotation copies the whole motif inward at smaller scale,
//!     which is what produces the nested medallions
//!   * a white background under a near-black palette
//!
//! That last one is worth explaining. Flames composite additively, so you
//! cannot simply "invert" one. But the tone mapper writes
//! `rgb + (ia * background) >> 8` where `ia = 255 - alpha`: dense pixels have
//! a high alpha, so almost no background bleeds through and the (dark) palette
//! colour wins, while sparse pixels are almost entirely background. A dark
//! palette over a white ground therefore renders dark-on-light naturally.
//!
//! Run: cargo run --example clockwork --release -- out.png [variant]

use flame_core::flame::{Affine, XForm};
use flame_core::genome::{Flame, Palette};
use flame_core::registry;
use flame_core::render::render;
use flame_core::rng::Rng;

fn xf(vars: &[(&str, f64)], coefs: Affine, color: f64, density: f64) -> XForm {
    let mut x = XForm::default();
    x.coefs = coefs;
    x.density = density;
    x.color = color;
    x.set_variations(
        vars.iter()
            .map(|(n, w)| (registry::create(n).unwrap_or_else(|| panic!("no {n}")), *w))
            .collect(),
    );
    x
}

/// A similarity: scale `s`, rotation `deg`, translation `(tx, ty)`.
fn sim(s: f64, deg: f64, tx: f64, ty: f64) -> Affine {
    let r = deg.to_radians();
    Affine { a: s * r.cos(), b: s * r.sin(), c: -s * r.sin(), d: s * r.cos(), e: tx, f: ty }
}

/// Near-black through mid grey — the structure colour. Kept off pure black so
/// the densest cores still read as ink rather than a hole.
fn ink() -> Palette {
    Palette(
        (0..256)
            .map(|i| {
                let t = i as f64 / 255.0;
                // Most of the range sits dark, with a little tonal separation
                // at the top so overlapping strata stay distinguishable.
                let v = (14.0 + 92.0 * t.powf(1.6)) as u8;
                [v, v, (v as f64 * 1.06).min(255.0) as u8]
            })
            .collect(),
    )
}

fn base(name: &str) -> Flame {
    let mut f = Flame::default();
    f.width = 1000;
    f.height = 1000;
    f.center = [0.0, 0.0];
    f.pixels_per_unit = 260.0;
    // White ground; the dark palette composites over it as described above.
    f.background = [255.0, 255.0, 255.0];
    f.brightness = 2.2;
    f.gamma = 3.0;
    f.vibrancy = 0.35;
    f.sample_density = 900.0;
    f.spatial_filter_radius = 0.55;
    f.palette = ink();
    f.name = name.into();
    f
}

fn clockwork() -> Flame {
    let mut f = base("clockwork");

    // The gear: high-power julian gives the radial teeth and spokes.
    let mut gear = xf(&[("julian", 1.0)], sim(1.0, 0.0, 0.0, 0.0), 0.12, 2.2);
    gear.set_variation_param("julian", "julian_power", 11.0);
    gear.set_variation_param("julian", "julian_dist", 1.0);

    // Concentric bands.
    let mut rings = xf(&[("rings2", 0.9), ("linear", 0.1)], sim(0.92, 6.0, 0.0, 0.0), 0.55, 1.1);
    rings.set_variation_param("rings2", "rings2_val", 0.22);

    // The contracting rotation that nests the motif inside itself.
    let nest = xf(&[("linear", 1.0)], sim(0.62, 27.0, 0.28, -0.12), 0.85, 1.4);

    // A spherical inversion turns some of the linework into medallions.
    let medallion = xf(&[("spherical", 1.0)], sim(0.85, -14.0, 0.35, 0.2), 0.35, 0.8);

    f.xforms = vec![gear, rings, nest, medallion];
    f
}

/// A denser, more chaotic sibling — more like the busiest regions of the
/// reference, where several ring systems overlap.
fn clockwork_dense() -> Flame {
    let mut f = base("clockwork_dense");
    f.pixels_per_unit = 210.0;
    f.brightness = 1.9;

    let mut gear = xf(&[("julian", 1.0)], sim(1.0, 0.0, 0.0, 0.0), 0.1, 2.4);
    gear.set_variation_param("julian", "julian_power", 15.0);

    let mut rings = xf(&[("rings2", 1.0)], sim(0.95, 3.0, 0.05, 0.0), 0.5, 1.3);
    rings.set_variation_param("rings2", "rings2_val", 0.14);

    let nest = xf(&[("linear", 1.0)], sim(0.7, 33.0, 0.22, 0.18), 0.8, 1.5);

    let mut bw = xf(&[("bwraps", 0.85), ("linear", 0.15)], sim(0.9, 11.0, -0.3, 0.1), 0.3, 0.9);
    bw.set_variation_param("bwraps", "bwraps_cellsize", 0.55);
    bw.set_variation_param("bwraps", "bwraps_space", 0.1);
    bw.set_variation_param("bwraps", "bwraps_inner_twist", 0.35);

    f.xforms = vec![gear, rings, nest, bw];
    f
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "clockwork.png".into());
    let variant = std::env::args().nth(2).unwrap_or_else(|| "a".into());

    let mut f = if variant == "b" { clockwork_dense() } else { clockwork() };

    // Overridable so tone can be swept quickly without a rebuild.
    f.width = env_f64("CW_SIZE", 1000.0) as usize;
    f.height = f.width;
    f.brightness = env_f64("CW_BRIGHT", f.brightness);
    f.gamma = env_f64("CW_GAMMA", f.gamma);
    f.sample_density = env_f64("CW_QUALITY", f.sample_density);
    f.spatial_filter_radius = env_f64("CW_FILTER", f.spatial_filter_radius);
    f.pixels_per_unit = env_f64("CW_PPU", f.pixels_per_unit);
    f.center = [env_f64("CW_CX", f.center[0]), env_f64("CW_CY", f.center[1])];
    f.vibrancy = env_f64("CW_VIB", f.vibrancy);

    let mut rng = Rng::new(0x5EED);
    f.prepare(&mut rng);

    let t = std::time::Instant::now();
    let img = render(&f, 0x5EED);
    println!("{} — {} ms", f.name, t.elapsed().as_millis());

    // Mean luminance tells us at a glance whether the tone is in the right
    // ballpark: the reference is light overall with dark structure.
    let mut sum = 0u64;
    for p in img.data.chunks(4) {
        sum += ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u64;
    }
    println!("mean luminance: {:.0}/255", sum as f64 / (img.width * img.height) as f64);

    let file = std::fs::File::create(&out).expect("create png");
    let mut enc =
        png::Encoder::new(std::io::BufWriter::new(file), img.width as u32, img.height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("header").write_image_data(&img.data).expect("write");
    println!("wrote {out}");
}
