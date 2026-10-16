//! Perceptual binary space partitioning of the sRGB gamut.
//!
//! The sRGB gamut *is* the unit cube in gamma-encoded RGB, so partitioning the cube with
//! axis-aligned cuts never spends a code on an impossible color. What makes the partition
//! perceptual is *where* the cuts go: each cell measures its own extent in a perceptual
//! [`Geometry`] and cuts the perceptually longest axis, optionally at the point that halves
//! the perceptual length.
//!
//! The split rule is a pure function of the cell's box, so encoder and decoder rebuild the
//! same tree on the fly and nothing needs to be stored. Every prefix of a code decodes to the
//! center of a (larger) cell, which gives progressive refinement for free; with
//! [`Length::MaxError`] the tree stops descending once a cell is perceptually small enough,
//! giving a prefix-free variable-length code with a per-color error target.

use crate::code::{Code, Codec};
use crate::color::{Srgb, clamp01, linear_to_lab, linear_to_oklab, srgb_to_linear};
use crate::metric::{delta_e_2000, dist};

/// The space in which cells are measured. Distances are Euclidean in the embedding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geometry {
    /// Oklab.
    Oklab,
    /// Oklab with Ottosson's lightness toe `L_r`, which tames the cube root's infinite slope
    /// at black and tracks CIELAB `L*`.
    OklabToe,
    /// CIELAB (D65), scaled by 1/100 so units are comparable to Oklab.
    Lab,
    /// Oklab with `L` weighted by 0.4, the weight that minimized p99 ΔE00 for the lattice codec.
    OklabW,
}

impl Geometry {
    #[inline]
    pub fn embed(self, c: Srgb) -> [f64; 3] {
        let lin = srgb_to_linear(c);
        match self {
            Geometry::Oklab => linear_to_oklab(lin),
            Geometry::OklabToe => {
                let [l, a, b] = linear_to_oklab(lin);
                [toe(l), a, b]
            }
            Geometry::Lab => linear_to_lab(lin).map(|v| v / 100.0),
            Geometry::OklabW => {
                let [l, a, b] = linear_to_oklab(lin);
                [0.4 * l, a, b]
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Geometry::Oklab => "ok",
            Geometry::OklabToe => "okt",
            Geometry::Lab => "lab",
            Geometry::OklabW => "okw",
        }
    }
}

/// Ottosson's `L -> L_r` toe (https://bottosson.github.io/posts/colorpicker/).
#[inline]
pub fn toe(l: f64) -> f64 {
    const K1: f64 = 0.206;
    const K2: f64 = 0.03;
    const K3: f64 = (1.0 + K1) / (1.0 + K2);
    let x = K3 * l - K1;
    0.5 * (x + (x * x + 4.0 * K2 * K3 * l).sqrt())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitRule {
    /// Widest RGB axis, cut at the midpoint. Equivalent to uniform RGB quantization; control.
    RgbMidpoint,
    /// Perceptually longest axis (max chord over the 4 parallel edges), cut at the RGB midpoint.
    Midpoint,
    /// Perceptually longest axis, cut where the longest edge's perceptual chord is halved.
    Balanced,
}

/// How the variable-length mode measures a cell's worst-case error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorMetric {
    /// Euclidean distance in the codec's geometry.
    Geometry,
    /// CIEDE2000.
    De2000,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Exactly `n` bits per color.
    Fixed(u32),
    /// Descend until the cell's estimated max error (largest distance from the cell center to
    /// a corner) is at most `eps`, or `max_depth` is reached.
    MaxError { eps: f64, metric: ErrorMetric, max_depth: u32 },
}

#[derive(Clone, Copy, Debug)]
pub struct PerceptualKd {
    pub geometry: Geometry,
    pub rule: SplitRule,
    pub length: Length,
}

/// A cell of the partition: an axis-aligned box in gamma-encoded sRGB plus the embedded
/// coordinates of its 8 corners. Corner index bit `k` set means "high end on axis `k`".
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
    emb: [[f64; 3]; 8],
}

impl Cell {
    pub fn root(g: Geometry) -> Cell {
        let (lo, hi) = ([0.0; 3], [1.0; 3]);
        Cell { lo, hi, emb: std::array::from_fn(|i| g.embed(Self::corner_of(lo, hi, i))) }
    }

    #[inline]
    fn corner_of(lo: [f64; 3], hi: [f64; 3], i: usize) -> Srgb {
        std::array::from_fn(|k| if i >> k & 1 == 1 { hi[k] } else { lo[k] })
    }

    pub fn corner(&self, i: usize) -> Srgb {
        Self::corner_of(self.lo, self.hi, i)
    }

    /// Longest perceptual chord among the 4 edges parallel to `axis`: (length, low corner).
    fn longest_edge(&self, axis: usize) -> (f64, usize) {
        let mut best = (-1.0, 0);
        for i in 0..8 {
            if i >> axis & 1 == 0 {
                let d = dist(self.emb[i], self.emb[i | 1 << axis]);
                if d > best.0 {
                    best = (d, i);
                }
            }
        }
        best
    }

    /// Choose the cut `(axis, position)` for this cell.
    pub fn split(&self, g: Geometry, rule: SplitRule) -> (usize, f64) {
        if rule == SplitRule::RgbMidpoint {
            let mut axis = 0;
            for k in 1..3 {
                if self.hi[k] - self.lo[k] > self.hi[axis] - self.lo[axis] {
                    axis = k;
                }
            }
            return (axis, 0.5 * (self.lo[axis] + self.hi[axis]));
        }
        let mut axis = 0;
        let mut edge = self.longest_edge(0);
        for k in 1..3 {
            let e = self.longest_edge(k);
            if e.0 > edge.0 {
                axis = k;
                edge = e;
            }
        }
        let mid = 0.5 * (self.lo[axis] + self.hi[axis]);
        if rule == SplitRule::Midpoint {
            return (axis, mid);
        }
        // Bisection for the point on the longest edge with equal chords to both ends. Both
        // chords are monotone along an edge in practice. 16 steps place the cut to within
        // 2^-16 of the cell width, far below anything that matters for error.
        let a = self.emb[edge.1];
        let b = self.emb[edge.1 | 1 << axis];
        let mut p = self.corner(edge.1);
        let (mut lo, mut hi) = (self.lo[axis], self.hi[axis]);
        for _ in 0..16 {
            let m = 0.5 * (lo + hi);
            p[axis] = m;
            let o = g.embed(p);
            if dist(a, o) < dist(o, b) {
                lo = m;
            } else {
                hi = m;
            }
        }
        let pos = 0.5 * (lo + hi);
        if pos > self.lo[axis] && pos < self.hi[axis] { (axis, pos) } else { (axis, mid) }
    }

    /// The child on the low (`right == false`) or high side of the cut.
    pub fn child(&self, g: Geometry, axis: usize, pos: f64, right: bool) -> Cell {
        let (mut lo, mut hi) = (self.lo, self.hi);
        if right {
            lo[axis] = pos;
        } else {
            hi[axis] = pos;
        }
        let mut emb = self.emb;
        for (i, e) in emb.iter_mut().enumerate() {
            // Corners on the moved face are new; the others are shared with the parent.
            if (i >> axis & 1 == 1) != right {
                *e = g.embed(Self::corner_of(lo, hi, i));
            }
        }
        Cell { lo, hi, emb }
    }

    /// Reconstruction point: the center of the RGB box.
    pub fn center(&self) -> Srgb {
        std::array::from_fn(|k| 0.5 * (self.lo[k] + self.hi[k]))
    }

    /// Estimated worst-case error when decoding to the center: the largest distance from the
    /// center to any corner. For small cells the error is close to an affine image of the
    /// box, whose farthest point from the center is a corner.
    pub fn radius(&self, g: Geometry, metric: ErrorMetric) -> f64 {
        let c = self.center();
        match metric {
            ErrorMetric::Geometry => {
                let ce = g.embed(c);
                self.emb.iter().map(|&e| dist(ce, e)).fold(0.0, f64::max)
            }
            ErrorMetric::De2000 => {
                let lab = |s: Srgb| linear_to_lab(srgb_to_linear(s));
                let cl = lab(c);
                (0..8).map(|i| delta_e_2000(cl, lab(self.corner(i)))).fold(0.0, f64::max)
            }
        }
    }
}

impl PerceptualKd {
    pub fn fixed(geometry: Geometry, rule: SplitRule, bits: u32) -> Self {
        PerceptualKd { geometry, rule, length: Length::Fixed(bits) }
    }

    pub fn max_error(geometry: Geometry, rule: SplitRule, metric: ErrorMetric, eps: f64, max_depth: u32) -> Self {
        PerceptualKd { geometry, rule, length: Length::MaxError { eps, metric, max_depth } }
    }

    /// Whether the tree stops at `cell`, which sits at `depth`.
    #[inline]
    fn is_leaf(&self, cell: &Cell, depth: u32) -> bool {
        match self.length {
            Length::Fixed(n) => depth >= n,
            Length::MaxError { eps, metric, max_depth } => {
                depth >= max_depth || cell.radius(self.geometry, metric) <= eps
            }
        }
    }

    /// Walk the tree along `code` and return the final cell.
    pub fn cell_for(&self, code: Code) -> Cell {
        let g = self.geometry;
        let mut cell = Cell::root(g);
        for i in 0..code.len {
            let (axis, pos) = cell.split(g, self.rule);
            cell = cell.child(g, axis, pos, code.bit(i));
        }
        cell
    }

    /// Decode a stream of concatenated variable-length codes. Returns the colors and the
    /// number of bits consumed. Trailing bits that don't complete a code are ignored.
    pub fn decode_stream(&self, bits: &[bool]) -> (Vec<Srgb>, usize) {
        let g = self.geometry;
        let mut out = Vec::new();
        let (mut i, mut consumed) = (0, 0);
        while i < bits.len() {
            let mut cell = Cell::root(g);
            let mut depth = 0;
            while !self.is_leaf(&cell, depth) && i < bits.len() {
                let (axis, pos) = cell.split(g, self.rule);
                cell = cell.child(g, axis, pos, bits[i]);
                i += 1;
                depth += 1;
            }
            if !self.is_leaf(&cell, depth) {
                break;
            }
            out.push(cell.center());
            consumed = i;
        }
        (out, consumed)
    }
}

impl Codec for PerceptualKd {
    fn name(&self) -> String {
        let rule = match self.rule {
            SplitRule::RgbMidpoint => "kd-rgb".to_string(),
            SplitRule::Midpoint => format!("kd-{}-mid", self.geometry.name()),
            SplitRule::Balanced => format!("kd-{}-bal", self.geometry.name()),
        };
        match self.length {
            Length::Fixed(n) => format!("{rule} {n}b"),
            Length::MaxError { eps, metric: ErrorMetric::Geometry, .. } => format!("{rule} ε={eps}"),
            Length::MaxError { eps, metric: ErrorMetric::De2000, .. } => format!("{rule} ΔE00≤{eps}"),
        }
    }

    fn encode(&self, c: Srgb) -> Code {
        let g = self.geometry;
        let c = clamp01(c);
        let mut cell = Cell::root(g);
        let mut code = Code::default();
        while !self.is_leaf(&cell, code.len) {
            let (axis, pos) = cell.split(g, self.rule);
            let right = c[axis] >= pos;
            code.push(right);
            cell = cell.child(g, axis, pos, right);
        }
        code
    }

    fn decode(&self, code: Code) -> Srgb {
        self.cell_for(code).center()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metric::delta_e_2000_srgb;

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 11) as f64 / (1u64 << 53) as f64
    }

    #[test]
    fn toe_matches_endpoints_and_is_monotone() {
        assert!(toe(0.0).abs() < 1e-12);
        assert!((toe(1.0) - 1.0).abs() < 1e-9);
        let mut prev = -1.0;
        for i in 0..=1000 {
            let v = toe(i as f64 / 1000.0);
            assert!(v > prev);
            prev = v;
        }
    }

    #[test]
    fn encoded_color_lies_in_its_cell() {
        for g in [Geometry::Oklab, Geometry::OklabToe, Geometry::Lab] {
            let codec = PerceptualKd::fixed(g, SplitRule::Balanced, 24);
            let mut s = 1;
            for _ in 0..1000 {
                let c = [lcg(&mut s), lcg(&mut s), lcg(&mut s)];
                let cell = codec.cell_for(codec.encode(c));
                for k in 0..3 {
                    assert!(cell.lo[k] <= c[k] && c[k] <= cell.hi[k]);
                }
            }
        }
    }

    #[test]
    fn rgb_midpoint_rule_matches_uniform_grid() {
        let codec = PerceptualKd::fixed(Geometry::Oklab, SplitRule::RgbMidpoint, 24);
        let c = [0.2, 0.55, 0.9];
        let d = codec.decode(codec.encode(c));
        for k in 0..3 {
            let want = ((c[k] * 256.0).floor() + 0.5) / 256.0;
            assert!((d[k] - want).abs() < 1e-12);
        }
    }

    #[test]
    fn every_prefix_decodes_inside_parent_cells() {
        let codec = PerceptualKd::fixed(Geometry::OklabToe, SplitRule::Balanced, 32);
        let code = codec.encode([0.18, 0.72, 0.36]);
        let mut prev = Cell::root(codec.geometry);
        for n in 0..=code.len {
            let cell = codec.cell_for(code.prefix(n));
            for k in 0..3 {
                assert!(prev.lo[k] <= cell.lo[k] && cell.hi[k] <= prev.hi[k]);
            }
            prev = cell;
        }
    }

    #[test]
    fn variable_length_stream_round_trips() {
        let codec = PerceptualKd::max_error(Geometry::OklabToe, SplitRule::Balanced, ErrorMetric::De2000, 1.0, 48);
        let colors = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.9, 0.1, 0.3], [0.02, 0.01, 0.03], [0.5, 0.5, 0.5]];
        let mut stream = Vec::new();
        let mut lens = Vec::new();
        for &c in &colors {
            let code = codec.encode(c);
            lens.push(code.len);
            stream.extend((0..code.len).map(|i| code.bit(i)));
        }
        let (decoded, used) = codec.decode_stream(&stream);
        assert_eq!(used, stream.len());
        assert_eq!(decoded.len(), colors.len());
        for (c, d) in colors.iter().zip(&decoded) {
            assert!(delta_e_2000_srgb(*c, *d) <= 1.0 + 1e-9, "{c:?} -> {d:?}");
        }
        assert!(lens.iter().any(|&l| l != lens[0]), "expected variable lengths: {lens:?}");
    }
}
