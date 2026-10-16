//! Flattening the gamut into a 2D grid ("unfolding").
//!
//! A [`Layout`] is a bijection between the ranks of a [`GamutLattice`] and cells of a `w × h`
//! grid, so a color becomes two coordinates `(u, v)`. Per-color bits are unchanged (the grid
//! has `2^n` cells for at most `2^n` points); what a layout decides is *locality*: whether
//! colors that are close end up in nearby cells, which matters for image compression,
//! interpolation of codes, and for how the grid looks as a color map.
//!
//! Most layouts are built in two steps: a method assigns every lattice point a continuous 2D
//! *target* position, then [`Layout::from_targets`] assigns points to cells by recursive median
//! bisection, which fills the grid evenly and keeps target-space neighbors together.

pub mod compress;
pub mod embed;
pub mod hilbert;
pub mod metrics;
pub mod octa;
pub mod okhsv;

use crate::codecs::lattice::GamutLattice;
use crate::color::{Srgb, oklab_to_linear, oklab_to_srgb_clipped};
use crate::stats::par_map;

const EMPTY: u32 = u32::MAX;

pub struct Layout {
    pub name: String,
    pub w: u32,
    pub h: u32,
    cell_of: Vec<u32>,
    rank_at: Vec<u32>,
}

impl Layout {
    /// Build from an explicit rank → cell table. Panics if two ranks share a cell.
    pub fn from_cells(name: String, w: u32, h: u32, cell_of: Vec<u32>) -> Layout {
        let mut rank_at = vec![EMPTY; (w * h) as usize];
        for (r, &c) in cell_of.iter().enumerate() {
            assert_eq!(rank_at[c as usize], EMPTY, "cell {c} used twice");
            rank_at[c as usize] = r as u32;
        }
        Layout { name, w, h, cell_of, rank_at }
    }

    /// Assign points to cells by recursive median bisection of their 2D targets: split the grid
    /// along its longer side, split the points at the matching target-coordinate median in
    /// proportion to the two halves' capacity, recurse.
    pub fn from_targets(name: String, w: u32, h: u32, targets: &[[f64; 2]]) -> Layout {
        assert!(targets.len() <= (w * h) as usize);
        assert!(targets.iter().all(|t| t[0].is_finite() && t[1].is_finite()), "non-finite target");
        let mut idx: Vec<u32> = (0..targets.len() as u32).collect();
        let mut cell_of = vec![0u32; targets.len()];
        bisect(&mut idx, targets, &mut cell_of, w, (0, 0, w, h));
        Layout::from_cells(name, w, h, cell_of)
    }

    #[inline]
    pub fn place(&self, rank: u64) -> (u32, u32) {
        let c = self.cell_of[rank as usize];
        (c % self.w, c / self.w)
    }

    #[inline]
    pub fn unplace(&self, u: u32, v: u32) -> Option<u64> {
        let r = self.rank_at[(v * self.w + u) as usize];
        (r != EMPTY).then_some(r as u64)
    }

    pub fn len(&self) -> usize {
        self.cell_of.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cell_of.is_empty()
    }

    /// Render the grid as an RGB image: each cell shows its color, empty cells a gray checker.
    pub fn render(&self, pts: &Points) -> image::RgbImage {
        image::RgbImage::from_fn(self.w, self.h, |u, v| match self.unplace(u, v) {
            Some(r) => image::Rgb(pts.srgb[r as usize].map(|x| (x * 255.0).round() as u8)),
            None => image::Rgb(if (u / 4 + v / 4) % 2 == 0 { [96; 3] } else { [160; 3] }),
        })
    }
}

fn bisect(idx: &mut [u32], t: &[[f64; 2]], cell_of: &mut [u32], gw: u32, (x0, y0, w, h): (u32, u32, u32, u32)) {
    let k = idx.len();
    if k == 0 {
        return;
    }
    if w * h == 1 {
        cell_of[idx[0] as usize] = y0 * gw + x0;
        return;
    }
    let axis = if w >= h { 0 } else { 1 };
    let (a, b) = if axis == 0 {
        ((x0, y0, w / 2, h), (x0 + w / 2, y0, w - w / 2, h))
    } else {
        ((x0, y0, w, h / 2), (x0, y0 + h / 2, w, h - h / 2))
    };
    let (cap_a, cap_b) = ((a.2 * a.3) as usize, (b.2 * b.3) as usize);
    // Proportional share keeps the unused cells spread out instead of piling up at the end.
    let n_a = ((k * cap_a) as f64 / (cap_a + cap_b) as f64).round() as usize;
    let n_a = n_a.clamp(k.saturating_sub(cap_b), cap_a.min(k));
    if n_a > 0 && n_a < k {
        idx.select_nth_unstable_by(n_a, |&p, &q| t[p as usize][axis].total_cmp(&t[q as usize][axis]));
    }
    let (left, right) = idx.split_at_mut(n_a);
    bisect(left, t, cell_of, gw, a);
    bisect(right, t, cell_of, gw, b);
}

/// Per-rank coordinates of all lattice points.
pub struct Points {
    pub oklab: Vec<[f64; 3]>,
    pub srgb: Vec<Srgb>,
}

impl Points {
    pub fn of(lat: &GamutLattice) -> Points {
        let ranks: Vec<u64> = (0..lat.count).collect();
        let oklab = par_map(&ranks, |&r| lat.oklab_of(r));
        let srgb = par_map(&oklab, |&o| oklab_to_srgb_clipped(o));
        Points { oklab, srgb }
    }

    pub fn len(&self) -> usize {
        self.oklab.len()
    }

    pub fn is_empty(&self) -> bool {
        self.oklab.is_empty()
    }
}

/// Square grid side for an `n`-bit lattice (`n` even).
pub fn side(n: u32) -> u32 {
    assert!(n.is_multiple_of(2), "flat layouts use square grids; n must be even");
    1 << (n / 2)
}

/// Control: ranks in row-major order (lattice columns are `L`-major, so rows sweep `b`).
pub fn raster(pts: &Points, n: u32) -> Layout {
    let s = side(n);
    Layout::from_cells("raster".into(), s, s, (0..pts.len() as u32).collect())
}

/// Ranks sorted along a space-filling curve in 3D, laid out along the same kind of curve in 2D.
pub fn curve(lat: &GamutLattice, pts: &Points, n: u32, hilbert: bool) -> Layout {
    let s = side(n);
    let ranks: Vec<u64> = (0..pts.len() as u64).collect();
    let idx = par_map(&ranks, |&r| lat.doubled_index(r));
    let lo: [i64; 3] = std::array::from_fn(|d| idx.iter().map(|p| p[d]).min().unwrap());
    let span = (0..3).map(|d| idx.iter().map(|p| p[d] - lo[d]).max().unwrap()).max().unwrap();
    let bits3 = 64 - (span as u64).leading_zeros();
    let keys = par_map(&idx, |p| {
        let q: [u32; 3] = std::array::from_fn(|d| (p[d] - lo[d]) as u32);
        if hilbert { hilbert::hilbert_index(q, bits3) } else { hilbert::morton_index(q, bits3) }
    });
    let mut order: Vec<u32> = (0..pts.len() as u32).collect();
    order.sort_unstable_by_key(|&r| keys[r as usize]);
    let mut cell_of = vec![0u32; pts.len()];
    for (i, &r) in order.iter().enumerate() {
        let [u, v] = if hilbert {
            hilbert::hilbert_point::<2>(i as u64, n / 2)
        } else {
            hilbert::morton_point::<2>(i as u64, n / 2)
        };
        cell_of[r as usize] = v * s + u;
    }
    Layout::from_cells(if hilbert { "hilbert-3d→2d" } else { "morton-3d→2d" }.into(), s, s, cell_of)
}

/// Group points into `tiles` slabs of equal count by `key`, lay the slabs out as a snake of
/// square tiles and place each point inside its tile at `inner` (in `[0,1]²`). With `mirror`,
/// every other tile is flipped so that touching edges of consecutive tiles show matching colors,
/// the same trick as folding a cube net.
fn tiled(name: String, n: u32, tiles: usize, key: &[f64], inner: &[[f64; 2]], mirror: bool) -> Layout {
    let cols = (tiles as f64).sqrt().round() as usize;
    assert_eq!(cols * cols, tiles, "tile count must be a square");
    let mut order: Vec<u32> = (0..key.len() as u32).collect();
    order.sort_unstable_by(|&p, &q| key[p as usize].total_cmp(&key[q as usize]));
    let mut slab = vec![0usize; key.len()];
    for (i, &r) in order.iter().enumerate() {
        slab[r as usize] = i * tiles / key.len();
    }
    let targets: Vec<[f64; 2]> = (0..key.len())
        .map(|r| {
            let t = slab[r];
            let row = t / cols;
            let mut col = t % cols;
            if row % 2 == 1 {
                col = cols - 1 - col;
            }
            let [mut fx, mut fy] = inner[r];
            if mirror {
                if col % 2 == 1 {
                    fx = 1.0 - fx;
                }
                if row % 2 == 1 {
                    fy = 1.0 - fy;
                }
            }
            [col as f64 + fx, row as f64 + fy]
        })
        .collect();
    let s = side(n);
    Layout::from_targets(name, s, s, &targets)
}

/// Normalize each slab's coordinates to fill `[0,1]²` (slabs differ a lot in extent).
fn normalize_per_slab(key: &[f64], tiles: usize, xy: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut order: Vec<u32> = (0..key.len() as u32).collect();
    order.sort_unstable_by(|&p, &q| key[p as usize].total_cmp(&key[q as usize]));
    let mut out = vec![[0.0; 2]; xy.len()];
    for chunk in order.chunks(key.len().div_ceil(tiles)) {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for &r in chunk {
            for d in 0..2 {
                lo[d] = lo[d].min(xy[r as usize][d]);
                hi[d] = hi[d].max(xy[r as usize][d]);
            }
        }
        // Same scale on both axes so tiles don't distort angles.
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-12);
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        for &r in chunk {
            let p = xy[r as usize];
            out[r as usize] = [0.5 + (p[0] - mid[0]) / span, 0.5 + (p[1] - mid[1]) / span];
        }
    }
    out
}

/// 3D-LUT style atlas: lightness slabs as tiles, `(a, b)` inside each tile.
pub fn slice_atlas(pts: &Points, n: u32, tiles: usize, mirror: bool) -> Layout {
    let key: Vec<f64> = pts.oklab.iter().map(|o| o[0]).collect();
    let ab: Vec<[f64; 2]> = pts.oklab.iter().map(|o| [o[1], o[2]]).collect();
    let inner = normalize_per_slab(&key, tiles, &ab);
    let name = format!("L-slices {tiles}{}", if mirror { " mirrored" } else { "" });
    tiled(name, n, tiles, &key, &inner, mirror)
}

/// Okhsv atlas: value slabs as tiles, hue/saturation as a disk inside each tile.
pub fn okhsv_atlas(pts: &Points, n: u32, tiles: usize, mirror: bool) -> Layout {
    let hsv = par_map(&pts.srgb, |&c| okhsv::srgb_to_okhsv(c));
    let key: Vec<f64> = hsv.iter().map(|x| x[2]).collect();
    let inner: Vec<[f64; 2]> = hsv
        .iter()
        .map(|&[h, s, _]| {
            let a = std::f64::consts::TAU * h;
            [0.5 + 0.5 * s * a.cos(), 0.5 + 0.5 * s * a.sin()]
        })
        .collect();
    let name = format!("okhsv-slices {tiles}{}", if mirror { " mirrored" } else { "" });
    tiled(name, n, tiles, &key, &inner, mirror)
}

/// Polar decomposition of the gamut around a center: for every point, its direction and its
/// relative depth `ρ = |p − c| / R(direction)`, where `R` is the distance to the first gamut
/// exit along that ray.
pub struct Polar {
    pub center: [f64; 3],
    pub dir: Vec<[f64; 3]>,
    pub rho: Vec<f64>,
    /// Points beyond the first exit of their ray (gamut not star-shaped there); `ρ` clamped.
    pub beyond_first_exit: usize,
}

fn in_gamut(o: [f64; 3]) -> bool {
    oklab_to_linear(o).iter().all(|v| (-1e-9..=1.0 + 1e-9).contains(v))
}

/// Distance from `c` to the first gamut exit along unit direction `d`.
pub fn first_exit(c: [f64; 3], d: [f64; 3]) -> f64 {
    let at = |t: f64| std::array::from_fn(|k| c[k] + t * d[k]);
    let step = 0.004;
    let mut t = 0.0;
    while in_gamut(at(t + step)) {
        t += step;
        if t > 2.0 {
            return t;
        }
    }
    let (mut lo, mut hi) = (t, t + step);
    for _ in 0..40 {
        let m = 0.5 * (lo + hi);
        if in_gamut(at(m)) {
            lo = m;
        } else {
            hi = m;
        }
    }
    lo
}

impl Polar {
    pub fn of(pts: &Points, center: [f64; 3]) -> Polar {
        let rows = par_map(&pts.oklab, |&o| {
            let d: [f64; 3] = std::array::from_fn(|k| o[k] - center[k]);
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if len < 1e-12 {
                return ([0.0, 0.0, 1.0], 0.0, false);
            }
            let u = d.map(|v| v / len);
            let r = first_exit(center, u);
            let rho = len / r;
            (u, rho.min(1.0), rho > 1.0 + 1e-6)
        });
        Polar {
            center,
            dir: rows.iter().map(|r| r.0).collect(),
            rho: rows.iter().map(|r| r.1).collect(),
            beyond_first_exit: rows.iter().filter(|r| r.2).count(),
        }
    }
}

/// Onion shells unfolded with the equal-area octahedral map, one tile per shell.
pub fn shells_octa(polar: &Polar, n: u32, tiles: usize, mirror: bool) -> Layout {
    let inner: Vec<[f64; 2]> = polar
        .dir
        .iter()
        .map(|&d| {
            let [u, v] = octa::sphere_to_square(d);
            [(u + 1.0) / 2.0, (v + 1.0) / 2.0]
        })
        .collect();
    let name = format!("shells-octa {tiles}{}", if mirror { " mirrored" } else { "" });
    tiled(name, n, tiles, &polar.rho, &inner, mirror)
}

/// Onion rings: shell `s` becomes an annulus of a disk; within it, the ring radius follows the
/// direction's elevation (equal-area) and the angle its azimuth. Elevation runs outward
/// north→south on even shells and south→north on odd ones, so consecutive shells meet at the
/// same pole and the radial direction has no seams at shell boundaries. `pole` selects the axis
/// (0 = L, 1 = a, 2 = b) whose two ends become the poles.
pub fn onion_rings(polar: &Polar, n: u32, shells: usize, pole: usize) -> Layout {
    let mut order: Vec<u32> = (0..polar.rho.len() as u32).collect();
    order.sort_unstable_by(|&p, &q| polar.rho[p as usize].total_cmp(&polar.rho[q as usize]));
    let mut shell = vec![0usize; order.len()];
    for (i, &r) in order.iter().enumerate() {
        shell[r as usize] = i * shells / order.len();
    }
    let (p1, p2) = ((pole + 1) % 3, (pole + 2) % 3);
    let targets: Vec<[f64; 2]> = (0..order.len())
        .map(|r| {
            let d = polar.dir[r];
            let e = (1.0 - d[pole]) / 2.0; // 0 at the north pole, 1 at the south pole; equal-area
            let s = shell[r];
            let e = if s.is_multiple_of(2) { e } else { 1.0 - e };
            // Annuli of equal area per shell: radius² linear in (s + e).
            let radius = ((s as f64 + e) / shells as f64).sqrt();
            let az = d[p2].atan2(d[p1]);
            [radius * az.cos(), radius * az.sin()]
        })
        .collect();
    let s = side(n);
    let axis = ["L", "a", "b"][pole];
    Layout::from_targets(format!("onion-rings {shells} pole={axis}"), s, s, &targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::lattice::LatticeKind;

    #[test]
    fn layouts_are_bijections() {
        let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, 12);
        let pts = Points::of(&lat);
        let center = [0.6, 0.0, 0.0];
        let polar = Polar::of(&pts, center);
        let layouts = [
            raster(&pts, 12),
            curve(&lat, &pts, 12, true),
            curve(&lat, &pts, 12, false),
            slice_atlas(&pts, 12, 16, true),
            okhsv_atlas(&pts, 12, 16, true),
            shells_octa(&polar, 12, 16, true),
            onion_rings(&polar, 12, 8, 0),
        ];
        for l in &layouts {
            assert_eq!(l.len(), pts.len(), "{}", l.name);
            for r in 0..pts.len() as u64 {
                let (u, v) = l.place(r);
                assert_eq!(l.unplace(u, v), Some(r), "{}", l.name);
            }
        }
    }
}
