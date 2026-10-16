//! Gamut-ranked lattices in Oklab.
//!
//! Uniform Oklab quantization wastes most of its codes: only about 21% of the Oklab bounding box
//! of the sRGB gamut is inside the gamut (≈2.3 bits per color). This codec puts a regular lattice
//! in (weighted) Oklab, keeps only the lattice points that lie inside the gamut, and numbers them
//! `0..count`. The code is that rank. With a body-centered cubic (BCC) lattice, the best
//! covering lattice in 3D, the worst-case error for a given point count drops further.
//!
//! The in-gamut points of every lattice column `(i, j)` are found exactly: along a line of
//! constant `L` and `a`, each linear RGB channel is a cubic polynomial in `b`, so the in-gamut set
//! is a union of intervals bounded by polynomial roots. Columns are stored as runs of `k`.

use crate::code::{Code, Codec};
use crate::color::{Srgb, clamp01, oklab_to_srgb_clipped, srgb_to_oklab};
use crate::codecs::baseline::oklab_gamut_bounds;
use crate::stats::par_map;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatticeKind {
    /// Simple cubic lattice `h·Z³`.
    Cubic,
    /// Body-centered cubic: `h·Z³ ∪ h·(Z³ + ½)`.
    Bcc,
}

impl LatticeKind {
    fn offsets(self) -> &'static [f64] {
        match self {
            LatticeKind::Cubic => &[0.0],
            LatticeKind::Bcc => &[0.0, 0.5],
        }
    }
}

/// The in-gamut points of one coset (`h·(Z³ + o)`), as runs of `k` per `(i, j)` column.
struct Coset {
    o: f64,
    i0: i64,
    ni: i64,
    j0: i64,
    nj: i64,
    /// `col_start[c]..col_start[c + 1]` are the runs of column `c = (i - i0) * nj + (j - j0)`.
    col_start: Vec<u32>,
    runs: Vec<(i32, i32)>,
    /// Rank of the first point of each run, plus the total count at the end.
    run_rank: Vec<u64>,
}

pub struct GamutLattice {
    pub kind: LatticeKind,
    /// Weight on `L` in the lattice space `(w·L, a, b)`. `w > 1` refines lightness.
    pub l_weight: f64,
    pub step: f64,
    pub bits: u32,
    cosets: Vec<Coset>,
    /// Rank offset of each coset.
    base: Vec<u64>,
    pub count: u64,
}

const BETA: [f64; 3] = [0.215_803_757_3, -0.063_854_172_8, -1.291_485_548_0];
const ALPHA_A: [f64; 3] = [0.396_337_777_4, -0.105_561_345_8, -0.089_484_177_5];
const M: [[f64; 3]; 3] = [
    [4.076_741_662_1, -3.307_711_591_3, 0.230_969_929_2],
    [-1.268_438_004_6, 2.609_757_401_1, -0.341_319_396_5],
    [-0.004_196_086_3, -0.703_418_614_7, 1.707_614_701_0],
];

/// The in-gamut intervals of `b` for fixed Oklab `L` and `a`, within `[b_lo, b_hi]`.
fn gamut_b_intervals(l: f64, a: f64, b_lo: f64, b_hi: f64) -> Vec<(f64, f64)> {
    // Each linear RGB channel is Σ_i M[c][i] · (α_i + β_i b)³, a cubic in b.
    let mut polys = [[0.0f64; 4]; 3];
    for i in 0..3 {
        let al = l + ALPHA_A[i] * a;
        let be = BETA[i];
        let cube = [al * al * al, 3.0 * al * al * be, 3.0 * al * be * be, be * be * be];
        for c in 0..3 {
            for d in 0..4 {
                polys[c][d] += M[c][i] * cube[d];
            }
        }
    }
    let eval = |p: &[f64; 4], x: f64| ((p[3] * x + p[2]) * x + p[1]) * x + p[0];

    let mut breaks = vec![b_lo, b_hi];
    for p in &polys {
        // Monotone pieces between the real roots of the derivative.
        let mut knots = vec![b_lo, b_hi];
        let (qa, qb, qc) = (3.0 * p[3], 2.0 * p[2], p[1]);
        if qa.abs() > 1e-300 {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc > 0.0 {
                let s = disc.sqrt();
                knots.extend([(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)]);
            }
        } else if qb.abs() > 1e-300 {
            knots.push(-qc / qb);
        }
        knots.retain(|&x| x >= b_lo && x <= b_hi);
        knots.sort_by(f64::total_cmp);
        for w in knots.windows(2) {
            for target in [0.0, 1.0] {
                let (mut x0, mut x1) = (w[0], w[1]);
                let (f0, f1) = (eval(p, x0) - target, eval(p, x1) - target);
                if f0 == 0.0 {
                    breaks.push(x0);
                }
                if f0.signum() * f1.signum() < 0.0 {
                    for _ in 0..60 {
                        let xm = 0.5 * (x0 + x1);
                        if (eval(p, xm) - target).signum() == f0.signum() {
                            x0 = xm;
                        } else {
                            x1 = xm;
                        }
                    }
                    breaks.push(0.5 * (x0 + x1));
                }
            }
        }
    }
    breaks.sort_by(f64::total_cmp);
    breaks.dedup();
    let inside = |x: f64| polys.iter().all(|p| (-1e-12..=1.0 + 1e-12).contains(&eval(p, x)));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for w in breaks.windows(2) {
        if inside(0.5 * (w[0] + w[1])) {
            match out.last_mut() {
                Some(last) if last.1 == w[0] => last.1 = w[1],
                _ => out.push((w[0], w[1])),
            }
        }
    }
    out
}

impl Coset {
    fn build(o: f64, h: f64, w: f64) -> Coset {
        let bounds = oklab_gamut_bounds();
        // Lattice index range covering the bounding box in lattice space (w·L, a, b).
        let range = |lo: f64, hi: f64| {
            let a = ((lo / h) - o).floor() as i64 - 1;
            let b = ((hi / h) - o).ceil() as i64 + 1;
            (a, b - a + 1)
        };
        let (i0, ni) = range(bounds[0][0] * w, bounds[0][1] * w);
        let (j0, nj) = range(bounds[1][0], bounds[1][1]);
        let (b_lo, b_hi) = (bounds[2][0], bounds[2][1]);
        let rows: Vec<i64> = (0..ni).collect();
        let per_row: Vec<Vec<Vec<(i32, i32)>>> = par_map(&rows, |&di| {
            let l = (i0 + di) as f64 * h / w + o * h / w;
            (0..nj)
                .map(|dj| {
                    let a = ((j0 + dj) as f64 + o) * h;
                    if !(0.0..=1.0).contains(&l) {
                        return Vec::new();
                    }
                    gamut_b_intervals(l, a, b_lo, b_hi)
                        .into_iter()
                        .filter_map(|(s0, s1)| {
                            let k0 = (s0 / h - o).ceil() as i64;
                            let k1 = (s1 / h - o).floor() as i64;
                            (k0 <= k1).then_some((k0 as i32, k1 as i32))
                        })
                        .collect()
                })
                .collect()
        });
        let mut col_start = Vec::with_capacity((ni * nj) as usize + 1);
        let mut runs = Vec::new();
        let mut run_rank = Vec::new();
        let mut rank = 0u64;
        for row in per_row {
            for col in row {
                col_start.push(runs.len() as u32);
                for r in col {
                    run_rank.push(rank);
                    rank += (r.1 - r.0 + 1) as u64;
                    runs.push(r);
                }
            }
        }
        col_start.push(runs.len() as u32);
        run_rank.push(rank);
        Coset { o, i0, ni, j0, nj, col_start, runs, run_rank }
    }

    fn count(&self) -> u64 {
        *self.run_rank.last().unwrap()
    }

    /// Rank of lattice point `(i, j, k)` within this coset, if it is in the gamut.
    fn rank(&self, i: i64, j: i64, k: i64) -> Option<u64> {
        let (di, dj) = (i - self.i0, j - self.j0);
        if di < 0 || di >= self.ni || dj < 0 || dj >= self.nj {
            return None;
        }
        let c = (di * self.nj + dj) as usize;
        for r in self.col_start[c] as usize..self.col_start[c + 1] as usize {
            let (k0, k1) = self.runs[r];
            if (k0 as i64..=k1 as i64).contains(&k) {
                return Some(self.run_rank[r] + (k - k0 as i64) as u64);
            }
        }
        None
    }

    fn unrank(&self, rank: u64) -> (i64, i64, i64) {
        let r = self.run_rank.partition_point(|&x| x <= rank) - 1;
        let c = self.col_start.partition_point(|&x| x as usize <= r) - 1;
        let (di, dj) = (c as i64 / self.nj, c as i64 % self.nj);
        let k = self.runs[r].0 as i64 + (rank - self.run_rank[r]) as i64;
        (self.i0 + di, self.j0 + dj, k)
    }
}

impl GamutLattice {
    fn with_step(kind: LatticeKind, l_weight: f64, step: f64, bits: u32) -> GamutLattice {
        let cosets: Vec<Coset> = kind.offsets().iter().map(|&o| Coset::build(o, step, l_weight)).collect();
        let mut base = Vec::new();
        let mut count = 0;
        for c in &cosets {
            base.push(count);
            count += c.count();
        }
        GamutLattice { kind, l_weight, step, bits, cosets, base, count }
    }

    /// The finest lattice of this kind and weight whose in-gamut point count fits in `bits`.
    pub fn new(kind: LatticeKind, l_weight: f64, bits: u32) -> GamutLattice {
        assert!(bits <= 34, "lattice tables grow as 2^bits");
        let target = (1u64 << bits) as f64;
        // Volume estimate: in-gamut Oklab volume ≈ 0.0546 (L·a·b units), scaled by w on L.
        let pts_per_cell = kind.offsets().len() as f64;
        let mut h = (0.0546 * l_weight * pts_per_cell / target).cbrt();
        let mut best: Option<GamutLattice> = None;
        for _ in 0..12 {
            let lat = Self::with_step(kind, l_weight, h, bits);
            let ratio = lat.count as f64 / target;
            if lat.count <= 1u64 << bits {
                let better = best.as_ref().is_none_or(|b| lat.count > b.count);
                if better {
                    best = Some(lat);
                }
                if ratio > 0.999 {
                    break;
                }
            }
            // Count scales like h^-3; aim slightly below the budget.
            h *= (ratio / 0.9995).cbrt();
        }
        best.expect("no lattice fits the bit budget")
    }

    /// Memory held by the rank tables.
    pub fn table_bytes(&self) -> usize {
        self.cosets
            .iter()
            .map(|c| c.col_start.len() * 4 + c.runs.len() * 8 + c.run_rank.len() * 8)
            .sum()
    }

    /// Rank of the in-gamut lattice point nearest to `c`.
    pub fn rank_of(&self, c: Srgb) -> u64 {
        let (ci, r) = self.nearest(self.to_space(srgb_to_oklab(clamp01(c))));
        self.base[ci] + r
    }

    fn locate(&self, rank: u64) -> (usize, (i64, i64, i64)) {
        let rank = rank.min(self.count - 1);
        let ci = self.base.partition_point(|&b| b <= rank) - 1;
        (ci, self.cosets[ci].unrank(rank - self.base[ci]))
    }

    /// Oklab coordinates of the lattice point with this rank (unweighted `L`).
    pub fn oklab_of(&self, rank: u64) -> [f64; 3] {
        let (ci, idx) = self.locate(rank);
        self.point(ci, idx)
    }

    /// Integer coordinates of the point on the doubled grid `2·(index + offset)`, so cubic and
    /// BCC points share one integer lattice (used for space-filling-curve orderings).
    pub fn doubled_index(&self, rank: u64) -> [i64; 3] {
        let (ci, (i, j, k)) = self.locate(rank);
        let o2 = (self.cosets[ci].o * 2.0) as i64;
        [2 * i + o2, 2 * j + o2, 2 * k + o2]
    }

    fn to_space(&self, o: [f64; 3]) -> [f64; 3] {
        [o[0] * self.l_weight, o[1], o[2]]
    }

    fn point(&self, coset: usize, (i, j, k): (i64, i64, i64)) -> [f64; 3] {
        let o = self.cosets[coset].o;
        let h = self.step;
        [(i as f64 + o) * h / self.l_weight, (j as f64 + o) * h, (k as f64 + o) * h]
    }

    /// Nearest in-gamut lattice point to `x` (lattice space): (coset, index, rank).
    fn nearest(&self, x: [f64; 3]) -> (usize, u64) {
        let h = self.step;
        let mut best = (f64::INFINITY, 0usize, 0u64);
        // Radius 0 is the per-coset rounding; the closest of those is the nearest lattice point,
        // so if it is in gamut we are done. Near the gamut boundary it may not be: widen ring by
        // ring until something is found, then search one more ring.
        let mut found_at = None;
        for radius in 0i64..=4 {
            for (ci, c) in self.cosets.iter().enumerate() {
                let center: [i64; 3] = std::array::from_fn(|d| (x[d] / h - c.o).round() as i64);
                for di in -radius..=radius {
                    for dj in -radius..=radius {
                        for dk in -radius..=radius {
                            if di.abs().max(dj.abs()).max(dk.abs()) != radius {
                                continue;
                            }
                            let idx = (center[0] + di, center[1] + dj, center[2] + dk);
                            if let Some(r) = c.rank(idx.0, idx.1, idx.2) {
                                let p = [(idx.0 as f64 + c.o) * h, (idx.1 as f64 + c.o) * h, (idx.2 as f64 + c.o) * h];
                                let d: f64 = (0..3).map(|q| (p[q] - x[q]).powi(2)).sum();
                                if d < best.0 {
                                    best = (d, ci, r);
                                }
                            }
                        }
                    }
                }
            }
            if best.0.is_finite() && found_at.is_none() {
                found_at = Some(radius);
            }
            match found_at {
                Some(0) => break,
                Some(r) if radius > r => break,
                _ => {}
            }
        }
        (best.1, best.2)
    }
}

impl Codec for GamutLattice {
    fn name(&self) -> String {
        let kind = match self.kind {
            LatticeKind::Cubic => "cubic",
            LatticeKind::Bcc => "bcc",
        };
        format!("lattice-{kind} w={} {}b", self.l_weight, self.bits)
    }

    fn encode(&self, c: Srgb) -> Code {
        let mut code = Code::default();
        code.push_bits(self.rank_of(c), self.bits);
        code
    }

    fn decode(&self, code: Code) -> Srgb {
        oklab_to_srgb_clipped(self.oklab_of(code.bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::oklab_to_linear;
    use crate::metric::delta_e_ok;

    #[test]
    fn intervals_agree_with_brute_force() {
        let b = oklab_gamut_bounds();
        for &(l, a) in &[(0.5, 0.0), (0.3, -0.1), (0.7, 0.2), (0.45, 0.05), (0.2, 0.02)] {
            let iv = gamut_b_intervals(l, a, b[2][0], b[2][1]);
            for s in 0..2000 {
                let bb = b[2][0] + (b[2][1] - b[2][0]) * (s as f64 + 0.5) / 2000.0;
                let inside = oklab_to_linear([l, a, bb]).iter().all(|v| (-1e-9..=1.0 + 1e-9).contains(v));
                let listed = iv.iter().any(|&(s0, s1)| s0 - 1e-9 <= bb && bb <= s1 + 1e-9);
                assert_eq!(inside, listed, "L={l} a={a} b={bb} {iv:?}");
            }
        }
    }

    #[test]
    fn rank_unrank_round_trip_and_fits_budget() {
        for kind in [LatticeKind::Cubic, LatticeKind::Bcc] {
            let lat = GamutLattice::new(kind, 1.0, 16);
            assert!(lat.count <= 1 << 16 && lat.count > (1 << 16) * 98 / 100, "{}", lat.count);
            for rank in (0..lat.count).step_by(97) {
                let code = Code { bits: rank, len: 16 };
                let c = lat.decode(code);
                assert_eq!(lat.encode(c), code, "rank {rank} -> {c:?}");
            }
        }
    }

    #[test]
    fn bcc_beats_cubic_worst_case() {
        let mut s = 3u64;
        let mut rnd = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let colors: Vec<Srgb> = (0..20_000).map(|_| [rnd(), rnd(), rnd()]).collect();
        let worst = |lat: &GamutLattice| {
            colors.iter().map(|&c| delta_e_ok(c, lat.decode(lat.encode(c)))).fold(0.0, f64::max)
        };
        let cubic = worst(&GamutLattice::new(LatticeKind::Cubic, 1.0, 18));
        let bcc = worst(&GamutLattice::new(LatticeKind::Bcc, 1.0, 18));
        assert!(bcc < cubic, "bcc {bcc} cubic {cubic}");
    }
}
