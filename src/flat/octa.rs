//! Equal-area octahedral map from the unit sphere to the square `[-1, 1]²`
//! (Clarberg, "Fast Equal-Area Mapping of the (Hemi)Sphere using SIMD", JGT 2008).
//!
//! The upper hemisphere fills the diamond `|u| + |v| ≤ 1`; the lower hemisphere is folded out
//! into the four corners. Only the forward direction is needed: layouts assign cells by
//! position, and decoding goes through the cell table.

use std::f64::consts::FRAC_PI_2;

/// Map a (not necessarily normalized, non-zero) direction to `[-1, 1]²`.
pub fn sphere_to_square(d: [f64; 3]) -> [f64; 2] {
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let [x, y, z] = d.map(|v| v / n);
    let r = (1.0 - z.abs()).max(0.0).sqrt();
    let phi = y.abs().atan2(x.abs()) / FRAC_PI_2;
    let mut v = phi * r;
    let mut u = r - v;
    if z < 0.0 {
        (u, v) = (1.0 - v, 1.0 - u);
    }
    [u.copysign(x), v.copysign(y)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Rng;

    #[test]
    fn uniform_sphere_fills_square_uniformly() {
        let mut rng = Rng::new(5);
        let bins = 8;
        let mut hist = vec![0u32; bins * bins];
        let n = 400_000;
        for _ in 0..n {
            // Uniform direction: z uniform in [-1, 1], azimuth uniform.
            let z = 2.0 * rng.f64() - 1.0;
            let a = std::f64::consts::TAU * rng.f64();
            let r = (1.0 - z * z).sqrt();
            let [u, v] = sphere_to_square([r * a.cos(), r * a.sin(), z]);
            assert!(u.abs() <= 1.0 + 1e-12 && v.abs() <= 1.0 + 1e-12);
            let bu = (((u + 1.0) / 2.0 * bins as f64) as usize).min(bins - 1);
            let bv = (((v + 1.0) / 2.0 * bins as f64) as usize).min(bins - 1);
            hist[bv * bins + bu] += 1;
        }
        let expect = n as f64 / (bins * bins) as f64;
        for &h in &hist {
            assert!((h as f64 - expect).abs() < 0.05 * expect, "bin {h} vs {expect}");
        }
    }
}
