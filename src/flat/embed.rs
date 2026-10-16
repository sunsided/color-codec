//! Kernel and manifold embeddings of the gamut into 2D: landmark RBF kernel PCA and landmark
//! Isomap (de Silva & Tenenbaum, 2003). The gamut is a solid, so these can only squash or tear
//! it; they are here to measure how badly.

use super::{Layout, Points, side};
use crate::codecs::lattice::GamutLattice;
use crate::data::Rng;
use crate::stats::par_map;

fn lattice_space(lat: &GamutLattice, pts: &Points) -> Vec<[f64; 3]> {
    pts.oklab.iter().map(|o| [o[0] * lat.l_weight, o[1], o[2]]).collect()
}

fn d2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

fn landmarks(n: usize, m: usize, seed: u64) -> Vec<usize> {
    let mut rng = Rng::new(seed);
    let mut idx: Vec<usize> = (0..n).collect();
    for i in 0..m {
        let j = i + (rng.next_u64() % (n - i) as u64) as usize;
        idx.swap(i, j);
    }
    idx.truncate(m);
    idx
}

/// Top `k` eigenpairs of a symmetric `n × n` matrix by power iteration with deflation.
fn top_eigs(m: &[f64], n: usize, k: usize) -> Vec<(f64, Vec<f64>)> {
    let mut found: Vec<(f64, Vec<f64>)> = Vec::new();
    let mut rng = Rng::new(42);
    for _ in 0..k {
        let mut v: Vec<f64> = (0..n).map(|_| rng.f64() - 0.5).collect();
        let mut lambda = 0.0;
        for _ in 0..500 {
            let mut w: Vec<f64> = (0..n).map(|i| (0..n).map(|j| m[i * n + j] * v[j]).sum()).collect();
            for (l, u) in &found {
                let dot: f64 = (0..n).map(|i| u[i] * v[i]).sum();
                for i in 0..n {
                    w[i] -= l * dot * u[i];
                }
            }
            let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            lambda = norm;
            v = w.into_iter().map(|x| x / norm).collect();
        }
        found.push((lambda, v));
    }
    found
}

/// Double-center a symmetric matrix in place; returns (row means, total mean) before centering.
fn double_center(m: &mut [f64], n: usize) -> (Vec<f64>, f64) {
    let rm: Vec<f64> = (0..n).map(|i| m[i * n..(i + 1) * n].iter().sum::<f64>() / n as f64).collect();
    let tm = rm.iter().sum::<f64>() / n as f64;
    for i in 0..n {
        for j in 0..n {
            m[i * n + j] += tm - rm[i] - rm[j];
        }
    }
    (rm, tm)
}

/// RBF kernel PCA on `m` landmarks, Nyström extension to all points.
pub fn kernel_pca(lat: &GamutLattice, pts: &Points, n_bits: u32, m: usize) -> Layout {
    let x = lattice_space(lat, pts);
    let lm = landmarks(x.len(), m, 7);
    let mut dists: Vec<f64> = Vec::new();
    for i in 0..m {
        for j in (i + 1)..m {
            dists.push(d2(x[lm[i]], x[lm[j]]).sqrt());
        }
    }
    dists.sort_by(f64::total_cmp);
    let sigma = dists[dists.len() / 2];
    let kern = |a: [f64; 3], b: [f64; 3]| (-d2(a, b) / (2.0 * sigma * sigma)).exp();
    let mut k = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            k[i * m + j] = kern(x[lm[i]], x[lm[j]]);
        }
    }
    let (rm, tm) = double_center(&mut k, m);
    let eigs = top_eigs(&k, m, 2);
    let targets = par_map(&x, |&p| {
        let kx: Vec<f64> = lm.iter().map(|&l| kern(p, x[l])).collect();
        let mean = kx.iter().sum::<f64>() / m as f64;
        std::array::from_fn(|c| {
            let (lambda, v) = &eigs[c];
            (0..m).map(|i| v[i] * (kx[i] - mean - rm[i] + tm)).sum::<f64>() / lambda.sqrt()
        })
    });
    let s = side(n_bits);
    Layout::from_targets(format!("kernel-PCA (RBF, {m} landmarks)"), s, s, &targets)
}

/// Landmark Isomap: kNN graph on landmarks, geodesics by Dijkstra, classical MDS, and
/// distance-based triangulation for all other points.
pub fn isomap(lat: &GamutLattice, pts: &Points, n_bits: u32, m: usize, knn: usize) -> Layout {
    let x = lattice_space(lat, pts);
    let lm = landmarks(x.len(), m, 9);
    // kNN graph among landmarks (symmetrized).
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m];
    for i in 0..m {
        let mut nb: Vec<(usize, f64)> = (0..m).filter(|&j| j != i).map(|j| (j, d2(x[lm[i]], x[lm[j]]).sqrt())).collect();
        nb.sort_by(|a, b| a.1.total_cmp(&b.1));
        for &(j, d) in &nb[..knn] {
            adj[i].push((j, d));
            adj[j].push((i, d));
        }
    }
    let idx: Vec<usize> = (0..m).collect();
    let geo: Vec<Vec<f64>> = par_map(&idx, |&src| {
        let mut dist = vec![f64::INFINITY; m];
        let mut heap = std::collections::BinaryHeap::new();
        dist[src] = 0.0;
        heap.push((std::cmp::Reverse(0u64), src));
        while let Some((std::cmp::Reverse(dk), u)) = heap.pop() {
            let du = f64::from_bits(dk);
            if du > dist[u] {
                continue;
            }
            for &(v, w) in &adj[u] {
                let nd = du + w;
                if nd < dist[v] {
                    dist[v] = nd;
                    heap.push((std::cmp::Reverse(nd.to_bits()), v));
                }
            }
        }
        dist
    });
    let maxd = geo.iter().flatten().filter(|d| d.is_finite()).fold(0.0f64, |a, &b| a.max(b));
    let mut b = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            let d = if geo[i][j].is_finite() { geo[i][j] } else { 2.0 * maxd };
            b[i * m + j] = -0.5 * d * d;
        }
    }
    let delta_mean: Vec<f64> = (0..m).map(|j| (0..m).map(|i| -2.0 * b[i * m + j]).sum::<f64>() / m as f64).collect();
    double_center(&mut b, m);
    let eigs = top_eigs(&b, m, 2);
    let targets = par_map(&x, |&p| {
        // Geodesic to every landmark via the 5 nearest landmarks.
        let mut near: Vec<(usize, f64)> = (0..m).map(|i| (i, d2(p, x[lm[i]]).sqrt())).collect();
        near.select_nth_unstable_by(5, |a, b| a.1.total_cmp(&b.1));
        let near = &near[..5];
        std::array::from_fn(|c| {
            let (lambda, v) = &eigs[c];
            (0..m)
                .map(|i| {
                    let g = near.iter().map(|&(a, d)| d + geo[a][i].min(2.0 * maxd)).fold(f64::INFINITY, f64::min);
                    0.5 * v[i] / lambda.sqrt() * (delta_mean[i] - g * g)
                })
                .sum::<f64>()
        })
    });
    let s = side(n_bits);
    Layout::from_targets(format!("isomap ({m} landmarks, k={knn})"), s, s, &targets)
}
