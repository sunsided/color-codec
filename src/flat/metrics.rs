//! Locality measures for 2D layouts.

use super::{Layout, Points};
use crate::codecs::lattice::GamutLattice;
use crate::color::oklab_to_linear;
use crate::color::oklab_to_srgb_clipped;
use crate::data::Rng;
use crate::metric::delta_e_2000_srgb;
use crate::stats::{Summary, par_map};

/// ΔE00 between the colors of horizontally/vertically adjacent filled cells: how smooth the
/// grid looks as a color map.
#[derive(Debug, Clone, Copy)]
pub struct Seams {
    pub mean: f64,
    pub p99: f64,
    pub max: f64,
    /// Share of adjacent pairs with ΔE00 > 5 (a clearly visible seam).
    pub over5: f64,
}

pub fn seams(l: &Layout, pts: &Points) -> Seams {
    let rows: Vec<u32> = (0..l.h).collect();
    let per_row = par_map(&rows, |&v| {
        let mut out = Vec::new();
        for u in 0..l.w {
            let Some(a) = l.unplace(u, v) else { continue };
            for (du, dv) in [(1, 0), (0, 1)] {
                if u + du < l.w
                    && v + dv < l.h
                    && let Some(b) = l.unplace(u + du, v + dv)
                {
                    out.push(delta_e_2000_srgb(pts.srgb[a as usize], pts.srgb[b as usize]));
                }
            }
        }
        out
    });
    let all: Vec<f64> = per_row.into_iter().flatten().collect();
    let over5 = all.iter().filter(|&&d| d > 5.0).count() as f64 / all.len() as f64;
    let s = Summary::of(all);
    Seams { mean: s.mean, p99: s.p99, max: s.max, over5 }
}

/// Where do two nearly identical colors land? For random lattice points, a perturbed color one
/// lattice step away is encoded; the 2D distance between the two cells is recorded.
#[derive(Debug, Clone, Copy)]
pub struct Tears {
    /// Distances in cells.
    pub median: f64,
    pub p99: f64,
    pub max: f64,
    /// Share of pairs more than 4 cells apart.
    pub rate: f64,
    /// Mean ΔE00 of the sampled pairs (to show they are near-identical colors).
    pub pair_de: f64,
}

pub fn tears(l: &Layout, lat: &GamutLattice, pts: &Points, samples: usize, seed: u64) -> Tears {
    let mut rng = Rng::new(seed);
    let mut pairs = Vec::new();
    while pairs.len() < samples {
        let r = (rng.next_u64() % pts.len() as u64) as usize;
        // Random offset in a ball of one lattice step, in lattice space (w·L, a, b).
        let d: [f64; 3] = loop {
            let d = std::array::from_fn(|_| 2.0 * rng.f64() - 1.0);
            if d.iter().map(|x| x * x).sum::<f64>() <= 1.0 {
                break d;
            }
        };
        let o = pts.oklab[r];
        let p = [o[0] + d[0] * lat.step / lat.l_weight, o[1] + d[1] * lat.step, o[2] + d[2] * lat.step];
        if oklab_to_linear(p).iter().all(|v| (0.0..=1.0).contains(v)) {
            pairs.push((r as u64, oklab_to_srgb_clipped(p)));
        }
    }
    let rows = par_map(&pairs, |&(r, c)| {
        let r2 = lat.rank_of(c);
        let (a, b) = (l.place(r), l.place(r2));
        let dist = ((a.0 as f64 - b.0 as f64).powi(2) + (a.1 as f64 - b.1 as f64).powi(2)).sqrt();
        (dist, delta_e_2000_srgb(pts.srgb[r as usize], pts.srgb[r2 as usize]))
    });
    let rate = rows.iter().filter(|r| r.0 > 4.0).count() as f64 / rows.len() as f64;
    let pair_de = rows.iter().map(|r| r.1).sum::<f64>() / rows.len() as f64;
    let s = Summary::of(rows.into_iter().map(|r| r.0).collect());
    Tears { median: s.p50, p99: s.p99, max: s.max, rate, pair_de }
}

/// Trustworthiness and continuity (Venna & Kaski) at `k`, estimated from `queries` random query
/// points. Trustworthiness penalizes 2D neighbors that are far apart in color; continuity
/// penalizes color neighbors that are far apart in 2D. 1.0 is perfect.
pub fn trust_continuity(l: &Layout, lat: &GamutLattice, pts: &Points, k: usize, queries: usize, seed: u64) -> (f64, f64) {
    let n = pts.len();
    let x: Vec<[f64; 3]> = pts.oklab.iter().map(|o| [o[0] * lat.l_weight, o[1], o[2]]).collect();
    let cell: Vec<(u32, u32)> = (0..n as u64).map(|r| l.place(r)).collect();
    let mut rng = Rng::new(seed);
    let qs: Vec<usize> = (0..queries).map(|_| (rng.next_u64() % n as u64) as usize).collect();
    // Deterministic pseudo-random tie-break for the many equal grid distances.
    let jitter = |j: usize| ((j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 * 1e-9;
    let rows = par_map(&qs, |&q| {
        let ranks = |dist: &dyn Fn(usize) -> f64| {
            let mut order: Vec<u32> = (0..n as u32).filter(|&j| j as usize != q).collect();
            order.sort_unstable_by(|&a, &b| dist(a as usize).total_cmp(&dist(b as usize)));
            let mut rank = vec![0u32; n];
            for (i, &j) in order.iter().enumerate() {
                rank[j as usize] = i as u32 + 1;
            }
            (order, rank)
        };
        let d3 = |j: usize| (0..3).map(|d| (x[q][d] - x[j][d]).powi(2)).sum::<f64>();
        let d2 = |j: usize| {
            let (a, b) = (cell[q], cell[j]);
            (a.0 as f64 - b.0 as f64).powi(2) + (a.1 as f64 - b.1 as f64).powi(2) + jitter(j)
        };
        let (o3, r3) = ranks(&d3);
        let (o2, r2) = ranks(&d2);
        let t: f64 = o2[..k].iter().filter(|&&j| r3[j as usize] as usize > k).map(|&j| (r3[j as usize] as usize - k) as f64).sum();
        let c: f64 = o3[..k].iter().filter(|&&j| r2[j as usize] as usize > k).map(|&j| (r2[j as usize] as usize - k) as f64).sum();
        (t, c)
    });
    let norm = 2.0 / (queries as f64 * k as f64 * (2.0 * n as f64 - 3.0 * k as f64 - 1.0));
    let t: f64 = rows.iter().map(|r| r.0).sum();
    let c: f64 = rows.iter().map(|r| r.1).sum();
    (1.0 - norm * t, 1.0 - norm * c)
}
