//! Renders two reference flames to PNG so the port can be checked by eye.
//!
//! The Sierpinski case is deliberately unmistakable: three half-scale linear
//! transforms have exactly one attractor, so if the chaos game, the xaos
//! selection tables, the camera mapping or the tone curve are wrong, it will
//! not look like a Sierpinski triangle.
//!
//! Run: cargo run --example render_demo --release

use flame_core::builtins::{Builtin, BuiltinVar};
use flame_core::flame::{Affine, XForm};
use flame_core::genome::{Flame, Palette};
use flame_core::render::render;
use flame_core::rng::Rng;
use flame_core::variation::Variation;

fn linear_xform(coefs: Affine, color: f64) -> XForm {
    let mut xf = XForm::default();
    xf.coefs = coefs;
    xf.density = 1.0;
    xf.color = color;
    xf.set_variations(vec![(
        Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
        1.0,
    )]);
    xf
}

/// A warm fire ramp: black -> red -> orange -> white.
fn fire_palette() -> Palette {
    Palette(
        (0..256)
            .map(|i| {
                let t = i as f64 / 255.0;
                let r = (t * 3.0).min(1.0);
                let g = ((t - 0.33) * 2.2).clamp(0.0, 1.0);
                let b = ((t - 0.72) * 3.5).clamp(0.0, 1.0);
                [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
            })
            .collect(),
    )
}

fn write_png(path: &str, img: &flame_core::render::Image) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width as u32, img.height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("header").write_image_data(&img.data).expect("write");
    println!("wrote {path} ({}x{})", img.width, img.height);
}

/// Three half-scale maps to the corners of the unit square.
fn sierpinski() -> Flame {
    let half = |e: f64, f: f64| Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e, f };
    let mut flame = Flame::default();
    flame.xforms = vec![
        linear_xform(half(0.0, 0.0), 0.0),
        linear_xform(half(0.5, 0.0), 0.5),
        linear_xform(half(0.25, 0.5), 1.0),
    ];
    flame.width = 512;
    flame.height = 512;
    // The attractor lives in the unit square, so centre on (0.5, 0.5) and fit
    // exactly one unit across the frame.
    flame.center = [0.5, 0.5];
    flame.pixels_per_unit = 512.0;
    flame.background = [0.0, 0.0, 0.0];
    flame.brightness = 4.0;
    flame.gamma = 4.0;
    flame.sample_density = 50.0;
    flame.palette = fire_palette();
    flame.name = "sierpinski".into();
    flame
}

/// A spherical/swirl flame — exercises the precalc path and non-linear maps.
fn spherical_swirl() -> Flame {
    let mut flame = Flame::default();

    let mut a = XForm::default();
    a.coefs = Affine { a: 0.8, b: 0.6, c: -0.6, d: 0.8, e: 0.0, f: 0.0 };
    a.density = 1.0;
    a.color = 0.0;
    a.set_variations(vec![
        (Box::new(BuiltinVar::new(Builtin::Spherical)) as Box<dyn Variation>, 1.0),
        (Box::new(BuiltinVar::new(Builtin::Swirl)) as Box<dyn Variation>, 0.3),
    ]);

    let mut b = XForm::default();
    b.coefs = Affine { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e: 0.6, f: 0.3 };
    b.density = 0.6;
    b.color = 1.0;
    b.set_variations(vec![(
        Box::new(BuiltinVar::new(Builtin::Linear)) as Box<dyn Variation>,
        1.0,
    )]);

    flame.xforms = vec![a, b];
    flame.width = 512;
    flame.height = 512;
    flame.center = [0.0, 0.0];
    flame.pixels_per_unit = 180.0;
    flame.brightness = 4.0;
    flame.gamma = 4.0;
    flame.vibrancy = 1.0;
    flame.sample_density = 100.0;
    flame.palette = fire_palette();
    flame.name = "spherical_swirl".into();
    flame
}

fn main() {
    let outdir = std::env::args().nth(1).unwrap_or_else(|| ".".into());

    for mut flame in [sierpinski(), spherical_swirl()] {
        let name = flame.name.clone();
        let mut rng = Rng::new(0x5EED);
        flame.prepare(&mut rng);

        let t = std::time::Instant::now();
        let img = render(&flame, 0x5EED);
        let ms = t.elapsed().as_millis();

        // A quick sanity signal: how much of the frame actually got hit.
        let lit = img.data.chunks(4).filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8).count();
        let pct = 100.0 * lit as f64 / (img.width * img.height) as f64;
        println!("{name}: {ms} ms, {pct:.1}% of pixels lit");

        write_png(&format!("{outdir}/{name}.png"), &img);
    }
}
