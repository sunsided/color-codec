//! Flattening experiments. Usage: `cargo run --release --bin flat -- <layouts|images|maps|all>`.
//! Markdown goes to stdout; maps are written to `results/maps/`, README figures (`maps`) to
//! `docs/images/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use color_codec::codecs::lattice::{GamutLattice, LatticeKind};
use color_codec::color::{Srgb, linear_to_oklab, oklab_to_srgb_clipped, srgb_to_linear};
use color_codec::flat::compress::{byte_planes, entropy, paeth_residuals, zstd_len};
use color_codec::flat::metrics::{seams, tears, trust_continuity};
use color_codec::flat::{self, Layout, Points, Polar, embed};
use color_codec::metric::delta_e_2000_srgb;
use color_codec::stats::{Summary, par_map};

fn mean_center(pts: &Points) -> [f64; 3] {
    let n = pts.len() as f64;
    std::array::from_fn(|k| pts.oklab.iter().map(|o| o[k]).sum::<f64>() / n)
}

fn layouts_for(lat: &GamutLattice, pts: &Points, polar: &Polar, n: u32, with_kernels: bool) -> Vec<Layout> {
    let mut v = vec![
        flat::raster(pts, n),
        flat::curve(lat, pts, n, false),
        flat::curve(lat, pts, n, true),
        flat::slice_atlas(pts, n, 16, false),
        flat::slice_atlas(pts, n, 16, true),
        flat::slice_atlas(pts, n, 64, true),
        flat::okhsv_atlas(pts, n, 16, true),
        flat::okhsv_atlas(pts, n, 64, true),
        flat::shells_octa(polar, n, 16, true),
        flat::onion_rings(polar, n, 8, 0),
        flat::onion_rings(polar, n, 32, 0),
        flat::onion_rings(polar, n, 16, 1),
    ];
    if with_kernels {
        v.push(embed::kernel_pca(lat, pts, n, 1500));
        v.push(embed::isomap(lat, pts, n, 1500, 10));
    }
    v
}

fn slug(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect::<String>().replace("--", "-")
}

fn layouts() {
    std::fs::create_dir_all("results/maps").unwrap();
    for n in [16u32, 20] {
        let t = Instant::now();
        let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, n);
        let pts = Points::of(&lat);
        let center = mean_center(&pts);
        let polar = Polar::of(&pts, center);
        println!(
            "## {n}-bit BCC lattice (w = 0.4): {} points on a {s}×{s} grid\n\nShell center (Oklab mean of the gamut points): L={:.3} a={:.3} b={:.3}. Points beyond the first gamut exit of their ray (gamut not star-shaped there): {} ({:.3}%).\n",
            pts.len(),
            center[0],
            center[1],
            center[2],
            polar.beyond_first_exit,
            100.0 * polar.beyond_first_exit as f64 / pts.len() as f64,
            s = flat::side(n),
        );
        let ls = layouts_for(&lat, &pts, &polar, n, n == 16);
        let trust = n == 16;
        println!("| layout | adjacent-cell ΔE00 mean / p99 | seams ΔE00>5 | near-twin distance median / p99 (cells) | twins >4 cells apart |{}\n|---|---:|---:|---:|---:|{}", if trust { " trust. / cont. (k=10) |" } else { "" }, if trust { "---:|" } else { "" });
        for l in &ls {
            let s = seams(l, &pts);
            let te = tears(l, &lat, &pts, 20_000, 3);
            let tc = if trust {
                let (tr, co) = trust_continuity(l, &lat, &pts, 10, 300, 5);
                format!(" {tr:.4} / {co:.4} |")
            } else {
                String::new()
            };
            println!(
                "| {} | {:.2} / {:.1} | {:.2}% | {:.1} / {:.0} | {:.1}% |{tc}",
                l.name,
                s.mean,
                s.p99,
                s.over5 * 100.0,
                te.median,
                te.p99,
                te.rate * 100.0
            );
            if n == 16 {
                l.render(&pts).save(format!("results/maps/{}.png", slug(&l.name))).unwrap();
            }
        }
        println!("\n(near-twin pairs: mean ΔE00 between the two decoded colors ≈ {:.2}; built and measured in {:.0}s)\n", tears(&ls[0], &lat, &pts, 2000, 3).pair_de, t.elapsed().as_secs_f64());
    }
}

fn webp_lossless_len(img: &image::RgbImage, tmp: &Path, tag: &str) -> usize {
    let png = tmp.join(format!("{tag}.png"));
    let webp = tmp.join(format!("{tag}.webp"));
    img.save(&png).unwrap();
    let ok = Command::new("convert")
        .arg(&png)
        .args(["-define", "webp:lossless=true", "-define", "webp:method=6", "-quality", "100"])
        .arg(&webp)
        .status()
        .expect("ImageMagick convert");
    assert!(ok.success());
    std::fs::metadata(&webp).unwrap().len() as usize
}

fn png_len(img: &image::RgbImage) -> usize {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, CompressionType::Best, FilterType::Adaptive)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.len()
}

fn to_rgb8(w: u32, h: u32, px: &[Srgb]) -> image::RgbImage {
    image::RgbImage::from_fn(w, h, |x, y| image::Rgb(px[(y * w + x) as usize].map(|v| (v * 255.0).round() as u8)))
}

/// The layouts' coder (Paeth per plane + zstd -19) applied to 8-bit RGB channels.
fn rgb_paeth_zstd_len(w: u32, h: u32, px: &[Srgb]) -> usize {
    let mut bytes = Vec::new();
    for ch in 0..3 {
        let plane: Vec<u16> = px.iter().map(|c| (c[ch] * 255.0).round() as u16).collect();
        bytes.extend(paeth_residuals(&plane, w as usize, h as usize).iter().map(|&r| r as u8));
    }
    zstd_len(&bytes)
}

fn images() {
    let n = 20;
    let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, n);
    let pts = Points::of(&lat);
    let polar = Polar::of(&pts, mean_center(&pts));
    let ls = [
        flat::raster(&pts, n),
        flat::curve(&lat, &pts, n, false),
        flat::curve(&lat, &pts, n, true),
        flat::slice_atlas(&pts, n, 16, true),
        flat::okhsv_atlas(&pts, n, 16, true),
        flat::shells_octa(&polar, n, 16, true),
        flat::onion_rings(&polar, n, 8, 0),
    ];
    let tmp = PathBuf::from("target/flat_tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut files: Vec<PathBuf> = std::fs::read_dir("/usr/share/backgrounds")
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("jpg" | "png")))
        .collect();
    files.sort();

    println!("## Images: bits per pixel\n\nImages from `/usr/share/backgrounds`, resized to 1024 px wide (Lanczos3), quantized to the 20-bit BCC lattice. *orig* columns are the 8-bit original (zero loss); every other column carries the same lattice quantization error (ΔE00 shown). Layout columns: (u, v) planes, Paeth-predicted, byte planes, zstd -19. *quant WebP* is the quantized image as RGB888 in WebP lossless, the standard codec at equal distortion.\n");
    let names: Vec<String> = ls.iter().map(|l| l.name.clone()).collect();
    println!(
        "| image | px | ΔE00 p99 / max | rank entropy | orig PNG | orig WebP | quant WebP | quant RGB Paeth+zstd | rank zstd | {} |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|{}",
        names.join(" | "),
        "---:|".repeat(names.len())
    );
    let mut interp: Vec<Vec<f64>> = vec![Vec::new(); ls.len()];
    let mut totals = vec![0.0f64; 6 + ls.len()];
    let mut rgb_interp: Vec<f64> = Vec::new();
    for (fi, f) in files.iter().enumerate() {
        let img = image::open(f).unwrap().to_rgb8();
        let (w0, h0) = img.dimensions();
        let (w, h) = (1024, (1024.0 * h0 as f64 / w0 as f64).round() as u32);
        let img = image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3);
        let px: Vec<Srgb> = img.pixels().map(|p| p.0.map(|v| v as f64 / 255.0)).collect();
        let npx = px.len() as f64;
        let ranks: Vec<u64> = par_map(&px, |&c| lat.rank_of(c));
        let decoded: Vec<Srgb> = ranks.iter().map(|&r| pts.srgb[r as usize]).collect();
        let idx: Vec<usize> = (0..px.len()).collect();
        let err = Summary::of(par_map(&idx, |&i| delta_e_2000_srgb(px[i], decoded[i])));
        let bpp = |bytes: usize| bytes as f64 * 8.0 / npx;
        let ent = entropy(ranks.iter().copied());
        let orig_png = bpp(png_len(&img));
        let orig_webp = bpp(webp_lossless_len(&img, &tmp, &format!("o{fi}")));
        let quant_webp = bpp(webp_lossless_len(&to_rgb8(w, h, &decoded), &tmp, &format!("q{fi}")));
        let rank_bytes: Vec<u8> = ranks.iter().flat_map(|&r| [(r >> 16) as u8, (r >> 8) as u8, r as u8]).collect();
        // Planar split of the 3 rank bytes, like the byte planes used for the layouts.
        let planar: Vec<u8> = (0..3).flat_map(|b| rank_bytes.iter().skip(b).step_by(3).copied().collect::<Vec<_>>()).collect();
        let rank_z = bpp(zstd_len(&planar));
        let quant_rgb = bpp(rgb_paeth_zstd_len(w, h, &decoded));
        let mut cells = vec![ent, orig_png, orig_webp, quant_webp, quant_rgb, rank_z];
        for y in (0..h).step_by(4) {
            for x in (0..w - 1).step_by(2) {
                let (i, j) = ((y * w + x) as usize, (y * w + x + 1) as usize);
                if ranks[i] == ranks[j] {
                    continue;
                }
                let oi = linear_to_oklab(srgb_to_linear(decoded[i]));
                let oj = linear_to_oklab(srgb_to_linear(decoded[j]));
                let truth = oklab_to_srgb_clipped(std::array::from_fn(|k| 0.5 * (oi[k] + oj[k])));
                // What a GPU does with RGB888: average the 8-bit values.
                let avg: Srgb = std::array::from_fn(|k| {
                    ((decoded[i][k] * 255.0).round() + (decoded[j][k] * 255.0).round()) / 2.0 / 255.0
                });
                rgb_interp.push(delta_e_2000_srgb(truth, avg));
            }
        }
        for (li, l) in ls.iter().enumerate() {
            let uv: Vec<(u32, u32)> = ranks.iter().map(|&r| l.place(r)).collect();
            let u: Vec<u16> = uv.iter().map(|p| p.0 as u16).collect();
            let v: Vec<u16> = uv.iter().map(|p| p.1 as u16).collect();
            let mut bytes = byte_planes(&paeth_residuals(&u, w as usize, h as usize));
            bytes.extend(byte_planes(&paeth_residuals(&v, w as usize, h as usize)));
            cells.push(bpp(zstd_len(&bytes)));
            // Interpolation: average the codes of horizontal neighbors, decode, compare with the
            // Oklab midpoint of the two decoded colors.
            for y in (0..h).step_by(4) {
                for x in (0..w - 1).step_by(2) {
                    let (i, j) = ((y * w + x) as usize, (y * w + x + 1) as usize);
                    if ranks[i] == ranks[j] {
                        continue;
                    }
                    let mu = ((uv[i].0 + uv[j].0) as f64 / 2.0).round() as u32;
                    let mv = ((uv[i].1 + uv[j].1) as f64 / 2.0).round() as u32;
                    let oi = linear_to_oklab(srgb_to_linear(decoded[i]));
                    let oj = linear_to_oklab(srgb_to_linear(decoded[j]));
                    let truth = oklab_to_srgb_clipped(std::array::from_fn(|k| 0.5 * (oi[k] + oj[k])));
                    let got = l.unplace(mu, mv).map(|r| pts.srgb[r as usize]);
                    interp[li].push(got.map_or(100.0, |g| delta_e_2000_srgb(truth, g)));
                }
            }
        }
        for (t, c) in totals.iter_mut().zip(&cells) {
            *t += c / files.len() as f64;
        }
        let name = f.file_stem().unwrap().to_string_lossy();
        let short: String = name.chars().take(24).collect();
        println!(
            "| {short} | {w}×{h} | {:.2} / {:.2} | {} |",
            err.p99,
            err.max,
            cells.iter().map(|c| format!("{c:.2}")).collect::<Vec<_>>().join(" | ")
        );
    }
    println!("| **mean** | | | {} |\n", totals.iter().map(|c| format!("**{c:.2}**")).collect::<Vec<_>>().join(" | "));

    println!("### Interpolating codes\n\nHorizontal neighbor pixels with different codes: average their (u, v), decode, compare with the Oklab midpoint of the two colors. An empty cell counts as a failure (ΔE00 = 100).\n\n| layout | pairs | ΔE00 median | p99 | share > 2 |\n|---|---:|---:|---:|---:|");
    {
        let over = rgb_interp.iter().filter(|&&d| d > 2.0).count() as f64 / rgb_interp.len() as f64;
        let n = rgb_interp.len();
        let s = Summary::of(rgb_interp);
        println!("| RGB888 (baseline: average the channels) | {n} | {:.2} | {:.1} | {:.1}% |", s.p50, s.p99, over * 100.0);
    }
    for (l, e) in ls.iter().zip(interp) {
        let over = e.iter().filter(|&&d| d > 2.0).count() as f64 / e.len() as f64;
        let n = e.len();
        let s = Summary::of(e);
        println!("| {} | {n} | {:.2} | {:.1} | {:.1}% |", l.name, s.p50, s.p99, over * 100.0);
    }
    println!();
}

/// Run ImageMagick `montage` over labeled images.
fn montage(items: &[(String, PathBuf)], tile: &str, out: &Path) {
    let mut cmd = Command::new("montage");
    cmd.args(["-background", "white", "-fill", "#222222", "-depth", "8"])
        .args(["-font", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "-pointsize", "16"]);
    for (label, path) in items {
        cmd.arg("-label").arg(label).arg(path);
    }
    let ok = cmd.args(["-tile", tile, "-geometry", "+10+10"]).arg(out).status().expect("ImageMagick montage");
    assert!(ok.success());
}

/// Figures for the README, written to `docs/images/`.
fn maps() {
    let out = PathBuf::from("docs/images");
    let tmp = PathBuf::from("target/flat_tmp");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();

    // The best map: every 20-bit lattice color exactly once, 1024×1024.
    let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, 20);
    let pts = Points::of(&lat);
    flat::slice_atlas(&pts, 20, 64, true).render(&pts).save(out.join("gamut-atlas-1024.png")).unwrap();

    // What mirroring does, at 512×512.
    let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, 18);
    let pts = Points::of(&lat);
    let mut items = Vec::new();
    for (mirror, label) in [(false, "16 lightness slices"), (true, "16 lightness slices, mirrored")] {
        let path = tmp.join(format!("mirror-{mirror}.png"));
        flat::slice_atlas(&pts, 18, 16, mirror).render(&pts).save(&path).unwrap();
        items.push((label.to_string(), path));
    }
    montage(&items, "2x1", &out.join("mirroring.png"));

    // The other nets, at 256×256.
    let lat = GamutLattice::new(LatticeKind::Bcc, 0.4, 16);
    let pts = Points::of(&lat);
    let polar = Polar::of(&pts, mean_center(&pts));
    let nets: [(&str, Layout); 6] = [
        ("Hilbert 3D→2D", flat::curve(&lat, &pts, 16, true)),
        ("Okhsv slices, mirrored", flat::okhsv_atlas(&pts, 16, 16, true)),
        ("octahedral shells", flat::shells_octa(&polar, 16, 16, true)),
        ("onion rings", flat::onion_rings(&polar, 16, 8, 0)),
        ("kernel PCA", embed::kernel_pca(&lat, &pts, 16, 1500)),
        ("Isomap", embed::isomap(&lat, &pts, 16, 1500, 10)),
    ];
    let items: Vec<(String, PathBuf)> = nets
        .iter()
        .map(|(label, l)| {
            let path = tmp.join(format!("{}.png", slug(label)));
            l.render(&pts).save(&path).unwrap();
            (label.to_string(), path)
        })
        .collect();
    montage(&items, "3x2", &out.join("nets.png"));
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    let t = Instant::now();
    match which.as_str() {
        "layouts" => layouts(),
        "images" => images(),
        "maps" => maps(),
        "all" => {
            layouts();
            images();
        }
        other => panic!("unknown experiment {other}"),
    }
    eprintln!("done in {:.1}s", t.elapsed().as_secs_f64());
}
