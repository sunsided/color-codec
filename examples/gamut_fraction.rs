//! Fraction of the Oklab bounding box of the sRGB gamut that lies inside the gamut.
use color_codec::codecs::baseline::oklab_gamut_bounds;
use color_codec::color::oklab_to_linear;
use color_codec::data::Rng;

fn main() {
    let b = oklab_gamut_bounds();
    let mut rng = Rng::new(7);
    let n = 10_000_000;
    let mut inside = 0;
    for _ in 0..n {
        let o: [f64; 3] = std::array::from_fn(|k| b[k][0] + rng.f64() * (b[k][1] - b[k][0]));
        if oklab_to_linear(o).iter().all(|v| (0.0..=1.0).contains(v)) {
            inside += 1;
        }
    }
    let f = inside as f64 / n as f64;
    println!("bounds {b:?}\nin-gamut fraction {f:.4} => {:.2} bits wasted", -f.log2());
}
