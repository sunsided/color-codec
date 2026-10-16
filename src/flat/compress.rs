//! Simple lossless coding of code planes, to measure how well a layout's locality compresses.

/// Paeth-predicted residuals of a `w × h` plane (wrapping arithmetic, as in PNG).
pub fn paeth_residuals(plane: &[u16], w: usize, h: usize) -> Vec<u16> {
    let mut out = vec![0u16; plane.len()];
    for y in 0..h {
        for x in 0..w {
            let at = |xx: usize, yy: usize| plane[yy * w + xx] as i32;
            let a = if x > 0 { at(x - 1, y) } else { 0 };
            let b = if y > 0 { at(x, y - 1) } else { 0 };
            let c = if x > 0 && y > 0 { at(x - 1, y - 1) } else { 0 };
            let p = a + b - c;
            let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
            let pred = if pa <= pb && pa <= pc {
                a
            } else if pb <= pc {
                b
            } else {
                c
            };
            out[y * w + x] = (at(x, y) - pred) as u16;
        }
    }
    out
}

/// Split 16-bit values into a high-byte plane followed by a low-byte plane.
pub fn byte_planes(v: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 2);
    out.extend(v.iter().map(|x| (x >> 8) as u8));
    out.extend(v.iter().map(|x| *x as u8));
    out
}

/// Size after zstd at level 19.
pub fn zstd_len(bytes: &[u8]) -> usize {
    zstd::bulk::compress(bytes, 19).expect("zstd").len()
}

/// Zero-order entropy of a symbol stream, in bits per symbol.
pub fn entropy<T: std::hash::Hash + Eq>(symbols: impl IntoIterator<Item = T>) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut n = 0usize;
    for s in symbols {
        *counts.entry(s).or_insert(0usize) += 1;
        n += 1;
    }
    counts.values().map(|&c| {
        let p = c as f64 / n as f64;
        -p * p.log2()
    }).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paeth_of_constant_plane_is_mostly_zero() {
        let plane = vec![1234u16; 64 * 64];
        let r = paeth_residuals(&plane, 64, 64);
        assert_eq!(r.iter().filter(|&&x| x != 0).count(), 1);
    }
}
