//! Experiments for the README. Usage: `cargo run --release --bin study -- <experiment>`
//! where experiment is one of `fixed`, `var`, `prog`, `speed`, `all`.
//! Markdown goes to stdout; `fixed` also writes `results/fixed.csv`.

use std::fmt::Write as _;
use std::time::Instant;

use color_codec::code::Codec;
use color_codec::codecs::baseline::{OklabQuant, RgbQuant, Spiral, oklab_gamut_bounds};
use color_codec::codecs::kd::{ErrorMetric, Geometry, PerceptualKd, SplitRule};
use color_codec::codecs::lattice::{GamutLattice, LatticeKind};
use color_codec::color::Srgb;
use color_codec::data::Dataset;
use color_codec::metric::delta_e_2000_srgb;
use color_codec::stats::{Summary, evaluate, par_map};

const N: usize = 100_000;
const CALIB_N: usize = 20_000;
const SEED: u64 = 0xC0105;

/// All `[x, y, z]` with `x + y + z == n`, each within 3 of `n / 3`.
fn allocations(n: u32) -> Vec<[u32; 3]> {
    let c = n / 3;
    let (lo, hi) = (c.saturating_sub(3).max(1), c + 3);
    let mut v = Vec::new();
    for x in lo..=hi {
        for y in lo..=hi {
            if x + y < n && (lo..=hi).contains(&(n - x - y)) {
                v.push([x, y, n - x - y]);
            }
        }
    }
    v
}

/// Pick the candidate with the lowest p99 CIEDE2000 on a calibration set (uniform Oklab).
fn best_of(cands: Vec<Box<dyn Codec>>) -> Box<dyn Codec> {
    let calib = Dataset::UniformOklab.sample(CALIB_N, SEED + 1);
    cands
        .into_iter()
        .map(|c| (evaluate(c.as_ref(), &calib).de2000.p99, c))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}

fn fixed_rate_codecs(n: u32) -> Vec<Box<dyn Codec>> {
    let boxed = |c: PerceptualKd| Box::new(c) as Box<dyn Codec>;
    vec![
        best_of(allocations(n).into_iter().map(|b| Box::new(RgbQuant { bits: b }) as Box<dyn Codec>).collect()),
        best_of(allocations(n).into_iter().map(|b| Box::new(OklabQuant { bits: b }) as Box<dyn Codec>).collect()),
        best_of((n / 3 - 3..=n / 3 + 3).map(|l| Box::new(Spiral { l_bits: l, t_bits: n - l }) as Box<dyn Codec>).collect()),
        boxed(PerceptualKd::fixed(Geometry::Oklab, SplitRule::RgbMidpoint, n)),
        boxed(PerceptualKd::fixed(Geometry::Oklab, SplitRule::Balanced, n)),
        boxed(PerceptualKd::fixed(Geometry::OklabW, SplitRule::Balanced, n)),
        Box::new(GamutLattice::new(LatticeKind::Cubic, 0.4, n)),
        Box::new(GamutLattice::new(LatticeKind::Bcc, 0.4, n)),
        Box::new(GamutLattice::new(LatticeKind::Bcc, 1.0, n)),
    ]
}

fn fixed() {
    println!("## Fixed rate\n\nN = {N} colors per dataset. RGB/Oklab/spiral bit splits chosen by p99 ΔE00 on a separate uniform-Oklab calibration set.\n");
    let sets: Vec<_> = Dataset::ALL.iter().map(|d| (*d, d.sample(N, SEED))).collect();
    let mut csv = String::from("bits,codec,dataset,ok_mean,ok_p99,ok_max,de00_mean,de00_p99,de00_max\n");
    for n in [16, 20, 24, 28, 32] {
        let codecs = fixed_rate_codecs(n);
        let names: Vec<_> = sets.iter().map(|(d, _)| d.name()).collect();
        let mut p99 = format!("### {n} bits: p99 ΔE00 / max ΔE00\n\n| codec | {} |\n|---|{}\n", names.join(" | "), "---:|".repeat(names.len()));
        for c in &codecs {
            let mut cells = Vec::new();
            for (d, colors) in &sets {
                let e = evaluate(c.as_ref(), colors);
                cells.push(format!("{:.3} / {:.3}", e.de2000.p99, e.de2000.max));
                writeln!(
                    csv,
                    "{n},{},{},{},{},{},{},{},{}",
                    c.name(),
                    d.name(),
                    e.ok.mean,
                    e.ok.p99,
                    e.ok.max,
                    e.de2000.mean,
                    e.de2000.p99,
                    e.de2000.max
                )
                .unwrap();
            }
            writeln!(p99, "| {} | {} |", c.name(), cells.join(" | ")).unwrap();
        }
        println!("{p99}");
    }
    std::fs::create_dir_all("results").unwrap();
    std::fs::write("results/fixed.csv", csv).unwrap();
}

/// Smallest budget in `9..=40` for which `make(n)`'s observed max ΔE00 on `colors` meets `target`.
fn min_bits(colors: &[Srgb], target: f64, make: impl Fn(u32) -> Vec<Box<dyn Codec>>) -> String {
    (9..=40)
        .find(|&n| make(n).iter().any(|c| evaluate(c.as_ref(), colors).de2000.max <= target))
        .map_or("> 40".into(), |n| n.to_string())
}

fn var() {
    println!("## Bits needed for a worst-case CIEDE2000 target\n\nkd rows: variable-length tree that stops once the cell's corner radius in ΔE00 is ≤ target (mean and max code length, observed error). Fixed-rate rows: smallest budget whose *observed* max ΔE00 on the same {N} colors meets the target (RGB/Oklab: best bit split).\n");
    let sets: Vec<_> = Dataset::ALL.iter().map(|d| (*d, d.sample(N, SEED))).collect();
    for target in [2.0, 1.0, 0.5] {
        println!("### max ΔE00 ≤ {target}\n\n| dataset | kd-ok-bal mean bits | kd max bits | kd obs. max ΔE00 | kd over target | RGB bits | Oklab-grid bits | lattice-bcc w=0.4 bits |\n|---|---:|---:|---:|---:|---:|---:|---:|");
        for (d, colors) in &sets {
            let kd = PerceptualKd::max_error(Geometry::Oklab, SplitRule::Balanced, ErrorMetric::De2000, target, 60);
            let rows = par_map(colors, |&c| {
                let code = kd.encode(c);
                (delta_e_2000_srgb(c, kd.decode(code)), code.len)
            });
            let over = rows.iter().filter(|r| r.0 > target).count() as f64 / rows.len() as f64;
            let s = Summary::of(rows.iter().map(|r| r.0).collect());
            let bits_mean = rows.iter().map(|r| r.1 as f64).sum::<f64>() / rows.len() as f64;
            let bits_max = rows.iter().map(|r| r.1).max().unwrap();
            let rgb = min_bits(colors, target, |n| {
                allocations(n).into_iter().map(|b| Box::new(RgbQuant { bits: b }) as Box<dyn Codec>).collect()
            });
            let okg = min_bits(colors, target, |n| {
                allocations(n).into_iter().map(|b| Box::new(OklabQuant { bits: b }) as Box<dyn Codec>).collect()
            });
            let lat = min_bits(colors, target, |n| vec![Box::new(GamutLattice::new(LatticeKind::Bcc, 0.4, n))]);
            println!(
                "| {} | {bits_mean:.2} | {bits_max} | {:.3} | {:.3}% | {rgb} | {okg} | {lat} |",
                d.name(),
                s.max,
                over * 100.0
            );
        }
        println!();
    }
}

fn prog() {
    println!("## Progressive refinement\n\nUniform-Oklab set. Share of one-bit refinements (prefix n → n+1, n ≥ 8) that *increase* ΔE00, and p99 ΔE00 by prefix length.\n\n| codec | worse refinements | p99 @8 | @12 | @16 | @20 | @24 | @28 |\n|---|---:|---:|---:|---:|---:|---:|---:|");
    let colors = Dataset::UniformOklab.sample(20_000, SEED);
    for (g, rule) in [
        (Geometry::Oklab, SplitRule::RgbMidpoint),
        (Geometry::Oklab, SplitRule::Balanced),
        (Geometry::OklabToe, SplitRule::Balanced),
        (Geometry::Lab, SplitRule::Balanced),
    ] {
        let codec = PerceptualKd::fixed(g, rule, 32);
        let per_color: Vec<Vec<f64>> = par_map(&colors, |&c| {
            let code = codec.encode(c);
            (0..=32).map(|n| delta_e_2000_srgb(c, codec.decode(code.prefix(n)))).collect()
        });
        let worse = per_color
            .iter()
            .flat_map(|e| e.windows(2).skip(8).map(|w| (w[1] > w[0] + 1e-12) as u32))
            .sum::<u32>() as f64
            / (per_color.len() * 24) as f64;
        let p99 = |n: usize| Summary::of(per_color.iter().map(|e| e[n]).collect()).p99;
        println!(
            "| {} | {:.1}% | {:.2} | {:.2} | {:.2} | {:.3} | {:.3} | {:.3} |",
            codec.name(),
            worse * 100.0,
            p99(8),
            p99(12),
            p99(16),
            p99(20),
            p99(24),
            p99(28)
        );
    }
    println!();
}

fn speed() {
    println!("## Speed (single thread, unoptimized f64 reference code)\n\n| codec | encode ns/color | decode ns/color |\n|---|---:|---:|");
    let colors: Vec<Srgb> = Dataset::UniformSrgb.sample(20_000, SEED);
    oklab_gamut_bounds();
    let codecs: Vec<Box<dyn Codec>> = vec![
        Box::new(RgbQuant { bits: [11, 11, 10] }),
        Box::new(OklabQuant { bits: [11, 11, 10] }),
        Box::new(Spiral { l_bits: 11, t_bits: 21 }),
        Box::new(PerceptualKd::fixed(Geometry::Oklab, SplitRule::Midpoint, 32)),
        Box::new(PerceptualKd::fixed(Geometry::Oklab, SplitRule::Balanced, 32)),
        Box::new(PerceptualKd::max_error(Geometry::Oklab, SplitRule::Balanced, ErrorMetric::De2000, 1.0, 60)),
        Box::new(GamutLattice::new(LatticeKind::Bcc, 0.4, 24)),
        Box::new(GamutLattice::new(LatticeKind::Bcc, 0.4, 32)),
    ];
    for c in &codecs {
        // Warm-up pass so the first codec doesn't pay for page faults and clock ramp-up.
        let codes: Vec<_> = colors.iter().map(|&x| c.encode(x)).collect();
        std::hint::black_box(&codes);
        let t = Instant::now();
        let codes: Vec<_> = colors.iter().map(|&x| c.encode(x)).collect();
        let enc = t.elapsed().as_nanos() as f64 / colors.len() as f64;
        let t = Instant::now();
        let mut acc = 0.0;
        for &code in &codes {
            acc += c.decode(code)[0];
        }
        let dec = t.elapsed().as_nanos() as f64 / colors.len() as f64;
        std::hint::black_box(acc);
        println!("| {} | {enc:.0} | {dec:.0} |", c.name());
    }
    println!();
    for bits in [16, 20, 24, 28, 32] {
        let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, bits);
        println!("- {}: {} points, rank tables {:.1} MiB", lat.name(), lat.count, lat.table_bytes() as f64 / 1048576.0);
    }
    println!();
}

fn lattice_sweep() {
    println!("## Lattice L-weight sweep (20 bits)\n\n| codec | points | uniform-Oklab p99 / max ΔE00 | uniform-sRGB p99 / max | gray p99 / max |\n|---|---:|---:|---:|---:|");
    let sets: Vec<_> = [Dataset::UniformOklab, Dataset::UniformSrgb, Dataset::Gray].iter().map(|d| d.sample(N / 2, SEED)).collect();
    for kind in [LatticeKind::Cubic, LatticeKind::Bcc] {
        for w in [0.3, 0.4, 0.5, 0.6, 1.0] {
            let lat = GamutLattice::new(kind, w, 20);
            let cells: Vec<_> = sets
                .iter()
                .map(|c| {
                    let e = evaluate(&lat, c);
                    format!("{:.3} / {:.3}", e.de2000.p99, e.de2000.max)
                })
                .collect();
            println!("| {} | {} | {} |", lat.name(), lat.count, cells.join(" | "));
        }
    }
    println!();
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    let t = Instant::now();
    match which.as_str() {
        "fixed" => fixed(),
        "var" => var(),
        "prog" => prog(),
        "speed" => speed(),
        "lattice" => lattice_sweep(),
        "all" => {
            fixed();
            var();
            prog();
            speed();
        }
        other => panic!("unknown experiment {other}"),
    }
    eprintln!("done in {:.1}s", t.elapsed().as_secs_f64());
}
