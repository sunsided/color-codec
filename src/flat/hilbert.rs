//! Hilbert and Morton (Z-order) curves in `N` dimensions.
//!
//! Hilbert uses John Skilling's transpose algorithm ("Programming the Hilbert curve", AIP Conf.
//! Proc. 707, 2004): coordinates of `bits` bits each map to an index of `N·bits` bits.

/// Hilbert index of `x` (each coordinate < 2^bits).
pub fn hilbert_index<const N: usize>(mut x: [u32; N], bits: u32) -> u64 {
    debug_assert!(N as u32 * bits <= 64);
    let m = 1u32 << (bits - 1);
    // Inverse undo.
    let mut q = m;
    while q > 1 {
        let p = q - 1;
        for i in 0..N {
            if x[i] & q != 0 {
                x[0] ^= p;
            } else {
                let t = (x[0] ^ x[i]) & p;
                x[0] ^= t;
                x[i] ^= t;
            }
        }
        q >>= 1;
    }
    // Gray encode.
    for i in 1..N {
        x[i] ^= x[i - 1];
    }
    let mut t = 0;
    let mut q = m;
    while q > 1 {
        if x[N - 1] & q != 0 {
            t ^= q - 1;
        }
        q >>= 1;
    }
    for v in &mut x {
        *v ^= t;
    }
    interleave(x, bits)
}

/// Coordinates of Hilbert index `h`.
pub fn hilbert_point<const N: usize>(h: u64, bits: u32) -> [u32; N] {
    let mut x = deinterleave::<N>(h, bits);
    let n = 2u32 << (bits - 1);
    // Gray decode.
    let t = x[N - 1] >> 1;
    for i in (1..N).rev() {
        x[i] ^= x[i - 1];
    }
    x[0] ^= t;
    // Undo excess work.
    let mut q = 2;
    while q != n {
        let p = q - 1;
        for i in (0..N).rev() {
            if x[i] & q != 0 {
                x[0] ^= p;
            } else {
                let t = (x[0] ^ x[i]) & p;
                x[0] ^= t;
                x[i] ^= t;
            }
        }
        q <<= 1;
    }
    x
}

/// Morton (Z-order) index: bits interleaved, coordinate 0 most significant within each group.
pub fn morton_index<const N: usize>(x: [u32; N], bits: u32) -> u64 {
    interleave(x, bits)
}

pub fn morton_point<const N: usize>(h: u64, bits: u32) -> [u32; N] {
    deinterleave(h, bits)
}

fn interleave<const N: usize>(x: [u32; N], bits: u32) -> u64 {
    let mut h = 0u64;
    for b in (0..bits).rev() {
        for v in x {
            h = (h << 1) | ((v >> b) & 1) as u64;
        }
    }
    h
}

fn deinterleave<const N: usize>(h: u64, bits: u32) -> [u32; N] {
    let mut x = [0u32; N];
    let mut shift = N as u32 * bits;
    for b in (0..bits).rev() {
        for v in &mut x {
            shift -= 1;
            *v |= (((h >> shift) & 1) as u32) << b;
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<const N: usize>(bits: u32) {
        let total = 1u64 << (N as u32 * bits);
        let mut prev: Option<[u32; N]> = None;
        for h in 0..total {
            let p = hilbert_point::<N>(h, bits);
            assert_eq!(hilbert_index(p, bits), h);
            if let Some(q) = prev {
                let d: u32 = (0..N).map(|i| p[i].abs_diff(q[i])).sum();
                assert_eq!(d, 1, "Hilbert steps must be unit moves: {q:?} -> {p:?}");
            }
            prev = Some(p);
            assert_eq!(morton_index(morton_point::<N>(h, bits), bits), h);
        }
    }

    #[test]
    fn hilbert_2d_round_trip_and_adjacency() {
        check::<2>(5);
    }

    #[test]
    fn hilbert_3d_round_trip_and_adjacency() {
        check::<3>(4);
    }
}
