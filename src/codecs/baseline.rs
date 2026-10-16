//! Baseline codecs: per-channel uniform quantization in sRGB or Oklab, and the Archimedean
//! spiral codec from the original "two coordinates" idea.

use std::f64::consts::TAU;
use std::sync::OnceLock;

use crate::code::{Code, Codec};
use crate::color::{Srgb, clamp01, oklab_to_srgb_clipped, srgb_to_oklab};

/// Uniform UNORM quantization of gamma-encoded sRGB, `bits[i]` bits for channel `i`.
///
/// `bits = [8, 8, 8]` is RGB888, `[5, 6, 5]` is RGB565, `[11, 11, 10]` matches the bit split
/// of R11G11B10 (but as UNORM, not as small floats).
pub struct RgbQuant {
    pub bits: [u32; 3],
}

impl Codec for RgbQuant {
    fn name(&self) -> String {
        format!("RGB {}/{}/{}", self.bits[0], self.bits[1], self.bits[2])
    }

    fn encode(&self, c: Srgb) -> Code {
        let mut code = Code::default();
        for i in 0..3 {
            let max = ((1u64 << self.bits[i]) - 1) as f64;
            code.push_bits((c[i].clamp(0.0, 1.0) * max).round() as u64, self.bits[i]);
        }
        code
    }

    fn decode(&self, code: Code) -> Srgb {
        let mut out = [0.0; 3];
        let mut shift = code.len;
        for i in 0..3 {
            shift -= self.bits[i];
            let q = (code.bits >> shift) & ((1u64 << self.bits[i]) - 1);
            out[i] = q as f64 / ((1u64 << self.bits[i]) - 1) as f64;
        }
        out
    }
}

/// Axis-aligned bounding box of the sRGB gamut in Oklab, found by sampling the cube surface.
pub fn oklab_gamut_bounds() -> [[f64; 2]; 3] {
    static BOUNDS: OnceLock<[[f64; 2]; 3]> = OnceLock::new();
    *BOUNDS.get_or_init(|| {
        let mut b = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
        let n = 512;
        for face in 0..6 {
            let (axis, val) = (face / 2, (face % 2) as f64);
            for i in 0..=n {
                for j in 0..=n {
                    let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                    let mut c = [0.0; 3];
                    c[axis] = val;
                    c[(axis + 1) % 3] = u;
                    c[(axis + 2) % 3] = v;
                    let o = srgb_to_oklab(c);
                    for k in 0..3 {
                        b[k][0] = b[k][0].min(o[k]);
                        b[k][1] = b[k][1].max(o[k]);
                    }
                }
            }
        }
        // Small margin so the extreme samples are not on a cell boundary.
        for r in &mut b {
            let m = (r[1] - r[0]) * 1e-6;
            r[0] -= m;
            r[1] += m;
        }
        b
    })
}

/// Uniform quantization of Oklab inside the sRGB gamut's bounding box, midpoint reconstruction,
/// clipped back into the sRGB cube on decode.
pub struct OklabQuant {
    pub bits: [u32; 3],
}

impl Codec for OklabQuant {
    fn name(&self) -> String {
        format!("Oklab {}/{}/{}", self.bits[0], self.bits[1], self.bits[2])
    }

    fn encode(&self, c: Srgb) -> Code {
        let o = srgb_to_oklab(clamp01(c));
        let bounds = oklab_gamut_bounds();
        let mut code = Code::default();
        for i in 0..3 {
            let n = 1u64 << self.bits[i];
            let [lo, hi] = bounds[i];
            let q = (((o[i] - lo) / (hi - lo)) * n as f64).floor().clamp(0.0, (n - 1) as f64);
            code.push_bits(q as u64, self.bits[i]);
        }
        code
    }

    fn decode(&self, code: Code) -> Srgb {
        let bounds = oklab_gamut_bounds();
        let mut o = [0.0; 3];
        let mut shift = code.len;
        for i in 0..3 {
            shift -= self.bits[i];
            let n = 1u64 << self.bits[i];
            let q = (code.bits >> shift) & (n - 1);
            let [lo, hi] = bounds[i];
            o[i] = lo + (q as f64 + 0.5) / n as f64 * (hi - lo);
        }
        oklab_to_srgb_clipped(o)
    }
}

/// The Archimedean spiral codec: `L` quantized uniformly, chroma plane addressed by a single
/// index `t` along `a + ib = C·s·exp(2πiKs)`, `s = sqrt(t/N)`, with `K = sqrt(N/π)` turns.
pub struct Spiral {
    pub l_bits: u32,
    pub t_bits: u32,
}

impl Spiral {
    const C: f64 = 0.4;

    fn turns(&self) -> f64 {
        ((1u64 << self.t_bits) as f64 / std::f64::consts::PI).sqrt()
    }

    fn point(&self, t: u64) -> [f64; 2] {
        let n = (1u64 << self.t_bits) as f64;
        let s = (t as f64 / n).sqrt();
        let phase = TAU * self.turns() * s;
        [Self::C * s * phase.cos(), Self::C * s * phase.sin()]
    }
}

impl Codec for Spiral {
    fn name(&self) -> String {
        format!("Spiral {}/{}", self.l_bits, self.t_bits)
    }

    fn encode(&self, c: Srgb) -> Code {
        let [l, a, b] = srgb_to_oklab(clamp01(c));
        let nl = 1u64 << self.l_bits;
        let ql = (l * nl as f64).floor().clamp(0.0, (nl - 1) as f64) as u64;

        let n = 1u64 << self.t_bits;
        let k_turns = self.turns();
        let r = a.hypot(b) / Self::C;
        let h = b.atan2(a).rem_euclid(TAU) / TAU;
        // Candidate revolutions around the estimate; pick the closest decoded point.
        let k0 = (k_turns * r - h).round() as i64;
        let mut best = (f64::INFINITY, 0u64);
        for k in (k0 - 1)..=(k0 + 1) {
            if k < 0 {
                continue;
            }
            let s = (k as f64 + h) / k_turns;
            let t_est = (n as f64 * s * s).round() as i64;
            for t in (t_est - 2)..=(t_est + 2) {
                if t < 0 || t >= n as i64 {
                    continue;
                }
                let [pa, pb] = self.point(t as u64);
                let d = (pa - a).powi(2) + (pb - b).powi(2);
                if d < best.0 {
                    best = (d, t as u64);
                }
            }
        }
        let mut code = Code::default();
        code.push_bits(ql, self.l_bits);
        code.push_bits(best.1, self.t_bits);
        code
    }

    fn decode(&self, code: Code) -> Srgb {
        let t = code.bits & ((1u64 << self.t_bits) - 1);
        let ql = (code.bits >> self.t_bits) & ((1u64 << self.l_bits) - 1);
        let l = (ql as f64 + 0.5) / (1u64 << self.l_bits) as f64;
        let [a, b] = self.point(t);
        oklab_to_srgb_clipped([l, a, b])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb888_is_exact_on_grid() {
        let q = RgbQuant { bits: [8, 8, 8] };
        let c = [10.0 / 255.0, 128.0 / 255.0, 1.0];
        let d = q.decode(q.encode(c));
        for i in 0..3 {
            assert!((d[i] - c[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn oklab_bounds_are_sane() {
        let b = oklab_gamut_bounds();
        assert!(b[0][0] < 1e-3 && (b[0][1] - 1.0).abs() < 1e-3);
        assert!(b[1][0] < -0.2 && b[1][1] > 0.27);
        assert!(b[2][0] < -0.3 && b[2][1] > 0.19);
    }
}
