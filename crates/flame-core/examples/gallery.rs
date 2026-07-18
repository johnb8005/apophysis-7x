//! Renders the README gallery.
//!
//! Each flame is chosen to exercise a different part of the port, so the
//! images double as a visual regression check: if the chaos game, a variation
//! family, or the tone curve breaks, these stop looking right.
//!
//! Run: cargo run --example gallery --release -- docs/images

use flame_core::flame::{Affine, XForm};
use flame_core::genome::{Flame, Palette};
use flame_core::registry;
use flame_core::render::render;
use flame_core::rng::Rng;

fn xform(vars: &[(&str, f64)], coefs: Affine, color: f64, density: f64) -> XForm {
    let mut xf = XForm::default();
    xf.coefs = coefs;
    xf.density = density;
    xf.color = color;
    xf.set_variations(
        vars.iter()
            .map(|(n, w)| {
                (registry::create(n).unwrap_or_else(|| panic!("unknown variation {n}")), *w)
            })
            .collect(),
    );
    xf
}

fn af(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Affine {
    Affine { a, b, c, d, e, f }
}

/// Smooth ramp through a set of control colours.
fn ramp(stops: &[[u8; 3]]) -> Palette {
    Palette(
        (0..256)
            .map(|i| {
                let t = i as f64 / 255.0 * (stops.len() - 1) as f64;
                let k = (t.floor() as usize).min(stops.len() - 2);
                let f = t - k as f64;
                let (a, b) = (stops[k], stops[k + 1]);
                [
                    (a[0] as f64 + (b[0] as f64 - a[0] as f64) * f) as u8,
                    (a[1] as f64 + (b[1] as f64 - a[1] as f64) * f) as u8,
                    (a[2] as f64 + (b[2] as f64 - a[2] as f64) * f) as u8,
                ]
            })
            .collect(),
    )
}

fn base(name: &str, ppu: f64, quality: f64, palette: Palette) -> Flame {
    let mut f = Flame::default();
    f.width = 640;
    f.height = 640;
    f.center = [0.0, 0.0];
    f.pixels_per_unit = ppu;
    f.brightness = 4.0;
    f.gamma = 4.0;
    f.sample_density = quality;
    f.palette = palette;
    f.name = name.into();
    f
}

fn main() {
    let outdir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&outdir).expect("create output directory");

    let fire = ramp(&[[6, 0, 12], [140, 12, 8], [240, 120, 10], [255, 230, 140], [255, 255, 255]]);
    let ice = ramp(&[[2, 6, 24], [16, 70, 150], [70, 170, 230], [190, 240, 255], [255, 255, 255]]);
    let ember = ramp(&[[10, 2, 10], [90, 10, 70], [220, 60, 90], [255, 170, 90], [255, 245, 210]]);
    let jade = ramp(&[[2, 12, 10], [10, 90, 70], [60, 190, 130], [190, 245, 200], [255, 255, 255]]);

    let mut flames: Vec<Flame> = Vec::new();

    // Sierpinski — the unmistakable correctness check.
    {
        let half = |e: f64, f: f64| af(0.5, 0.0, 0.0, 0.5, e, f);
        let mut fl = base("sierpinski", 585.0, 120.0, fire.clone());
        fl.center = [0.5, 0.5];
        fl.xforms = vec![
            xform(&[("linear", 1.0)], half(0.0, 0.0), 0.0, 1.0),
            xform(&[("linear", 1.0)], half(0.5, 0.0), 0.5, 1.0),
            xform(&[("linear", 1.0)], half(0.25, 0.5), 1.0, 1.0),
        ];
        flames.push(fl);
    }

    // Spherical + swirl — the classic flame look.
    {
        let mut fl = base("spiral", 215.0, 600.0, ember.clone());
        fl.xforms = vec![
            xform(
                &[("spherical", 1.0), ("swirl", 0.35)],
                af(0.82, 0.58, -0.58, 0.82, 0.0, 0.0),
                0.0,
                1.0,
            ),
            xform(&[("linear", 1.0)], af(0.48, 0.0, 0.0, 0.48, 0.62, 0.3), 1.0, 0.55),
        ];
        flames.push(fl);
    }

    // julian at power 5 — the plugin dispatch kernels, with visible 5-fold
    // symmetry that makes a wrong implementation obvious.
    {
        let mut fl = base("julian", 180.0, 600.0, ice.clone());
        let mut a = xform(
            &[("julian", 1.0), ("pre_spherical", 1.0)],
            af(0.92, 0.36, -0.36, 0.92, 0.08, 0.0),
            0.0,
            1.0,
        );
        a.set_variation_param("julian", "julian_power", 5.0);
        fl.xforms =
            vec![a, xform(&[("curl", 1.0)], af(0.6, 0.0, 0.0, 0.6, -0.35, 0.25), 1.0, 0.5)];
        flames.push(fl);
    }

    // bwraps — the cell-warp family, including its pass-through fallback.
    {
        let mut fl = base("bwraps", 185.0, 600.0, jade.clone());
        let mut a = xform(
            &[("bwraps", 1.0), ("linear", 0.25)],
            af(0.75, 0.25, -0.25, 0.75, 0.0, 0.0),
            0.0,
            1.0,
        );
        a.set_variation_param("bwraps", "bwraps_cellsize", 0.7);
        a.set_variation_param("bwraps", "bwraps_space", 0.15);
        a.set_variation_param("bwraps", "bwraps_inner_twist", 0.6);
        fl.xforms = vec![
            a,
            xform(&[("spherical", 1.0)], af(0.5, 0.0, 0.0, 0.5, 0.4, 0.35), 0.85, 0.45),
        ];
        flames.push(fl);
    }

    for mut fl in flames {
        let name = fl.name.clone();
        let mut rng = Rng::new(0x5EED);
        fl.prepare(&mut rng);

        let t = std::time::Instant::now();
        let img = render(&fl, 0x5EED);
        let ms = t.elapsed().as_millis();

        let lit = img.data.chunks(4).filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8).count();
        let pct = 100.0 * lit as f64 / (img.width * img.height) as f64;

        let path = format!("{outdir}/{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut enc =
            png::Encoder::new(std::io::BufWriter::new(file), img.width as u32, img.height as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("header").write_image_data(&img.data).expect("write");

        println!("{name}: {ms} ms, {pct:.1}% lit -> {path}");
    }
}
