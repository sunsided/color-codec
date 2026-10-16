//! Deterministic test color sets.

use crate::color::{Srgb, linear_to_srgb, oklab_to_linear};
use crate::codecs::baseline::oklab_gamut_bounds;

/// SplitMix64: tiny, fast, reproducible.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dataset {
    /// Uniform in the gamma-encoded sRGB cube.
    UniformSrgb,
    /// Uniform in Oklab volume, restricted to the sRGB gamut (rejection sampling).
    UniformOklab,
    /// Like `UniformOklab`, but only `L < 0.3`.
    Dark,
    /// Cube surface: at least one channel at 0 or 1 (saturated / extreme colors).
    Surface,
    /// Neutral grays.
    Gray,
}

impl Dataset {
    pub const ALL: [Dataset; 5] =
        [Dataset::UniformSrgb, Dataset::UniformOklab, Dataset::Dark, Dataset::Surface, Dataset::Gray];

    pub fn name(self) -> &'static str {
        match self {
            Dataset::UniformSrgb => "uniform-sRGB",
            Dataset::UniformOklab => "uniform-Oklab",
            Dataset::Dark => "dark (L<0.3)",
            Dataset::Surface => "gamut surface",
            Dataset::Gray => "gray",
        }
    }

    pub fn sample(self, n: usize, seed: u64) -> Vec<Srgb> {
        let mut rng = Rng::new(seed ^ (self as u64).wrapping_mul(0x1234_5678_9ABC_DEF1));
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let c = match self {
                Dataset::UniformSrgb => Some([rng.f64(), rng.f64(), rng.f64()]),
                Dataset::UniformOklab => oklab_in_gamut(&mut rng, 1.0),
                Dataset::Dark => oklab_in_gamut(&mut rng, 0.3),
                Dataset::Surface => {
                    let mut c = [rng.f64(), rng.f64(), rng.f64()];
                    let axis = (rng.next_u64() % 3) as usize;
                    c[axis] = (rng.next_u64() & 1) as f64;
                    Some(c)
                }
                Dataset::Gray => {
                    let v = rng.f64();
                    Some([v, v, v])
                }
            };
            out.extend(c);
        }
        out
    }
}

fn oklab_in_gamut(rng: &mut Rng, l_max: f64) -> Option<Srgb> {
    let b = oklab_gamut_bounds();
    let o = [
        rng.f64() * l_max,
        b[1][0] + rng.f64() * (b[1][1] - b[1][0]),
        b[2][0] + rng.f64() * (b[2][1] - b[2][0]),
    ];
    let lin = oklab_to_linear(o);
    lin.iter().all(|&v| (0.0..=1.0).contains(&v)).then(|| linear_to_srgb(lin))
}
