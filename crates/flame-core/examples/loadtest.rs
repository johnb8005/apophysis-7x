//! Load a `.flame` file, report how it parsed, and render it to PNG.
//!
//! Useful for checking real-world files against the port — in particular the
//! new_linear synthesis, since the printed variation list shows whether each
//! xform gained a synthesised `flatten`.
//!
//! Run: cargo run --example loadtest --release -- input.flame output.png

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let r = flame_core::load::load(&text);
    println!("flames: {}", r.flames.len());
    for w in &r.warnings { println!("  warning: {} — {}", w.flame, w.message); }
    let mut f = r.flames.into_iter().next().unwrap();
    println!("name={} xforms={} scale={} quality={}", f.name, f.xforms.len(), f.pixels_per_unit, f.sample_density);
    for (i, xf) in f.xforms.iter().enumerate() {
        let names: Vec<&str> = xf.variations().iter().map(|(v,_)| v.name()).collect();
        println!("  xform {i}: weight={} vars={:?}", xf.density, names);
    }
    let mut rng = flame_core::rng::Rng::new(0x5EED);
    f.prepare(&mut rng);
    let img = flame_core::render::render(&f, 0x5EED);
    let lit = img.data.chunks(4).filter(|p| p[0]>8||p[1]>8||p[2]>8).count();
    println!("rendered {}x{}, {:.1}% lit", img.width, img.height, 100.0*lit as f64/(img.width*img.height) as f64);
    let file = std::fs::File::create(std::env::args().nth(2).unwrap()).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), img.width as u32, img.height as u32);
    e.set_color(png::ColorType::Rgba); e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&img.data).unwrap();
}
