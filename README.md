# color-codec

A research crate that tests how few bits a single sRGB color needs when "visually the same" is
good enough. It started from two ideas: express a color as two coordinates (a spiral through the
color wheel), then as a bitstream of left/right decisions through the gamut. Both were tested
here against plain quantization, together with a third design that came out of the experiments.

**Headline:** a body-centered cubic (BCC) lattice in lightness-weighted Oklab, with codes assigned
only to the lattice points inside the sRGB gamut, has about half of RGB888's error at the same
24 bits (p99 ΔE00 0.22 vs 0.45; worst case 0.34 vs 0.93). Guaranteeing a worst case of
**ΔE00 ≤ 1 (≈ 1 JND) takes 20 bits instead of RGB's 24**. A perceptual binary tree, the
left/right bitstream idea, gets the same guarantee at **~19 bits on average** as a
prefix-free, variable-length code.

Flattening the gamut onto a 2D plane ("unfolding", kernel/manifold embeddings) gives a good
full-gamut color map. It does **not** give fewer bits: on images every 2D layout needs ~24% more
bits than plain RGB under the same coder, because a 3D volume can only reach 2D by tearing.
See [Flattening](#flattening-the-gamut-into-2d-unfolding).

## Codecs

| Codec | Module | Idea |
|---|---|---|
| `RgbQuant` | `codecs::baseline` | Uniform UNORM quantization of gamma-encoded sRGB (RGB888, RGB565, …). |
| `OklabQuant` | `codecs::baseline` | Uniform grid over the Oklab bounding box of the gamut. |
| `Spiral` | `codecs::baseline` | `(L, t)`: quantized lightness plus a position on an Archimedean spiral in the a/b plane. |
| `PerceptualKd` | `codecs::kd` | Binary partition of the sRGB cube. Each bit picks a side of a cut placed by Oklab geometry. Progressive; optional variable length with an error target. |
| `GamutLattice` | `codecs::lattice` | Cubic or BCC lattice in `(w·L, a, b)`. Only in-gamut points get codes; the code is their rank. |

### Why the binary tree partitions the RGB cube, not Oklab

The sRGB gamut is exactly the unit cube in RGB, so a tree that cuts the cube never spends a code
on an impossible color. In Oklab only **20.8%** of the gamut's bounding box is inside the gamut
(`cargo run --release --example gamut_fraction`), so a plain Oklab grid wastes **2.27 bits per
color**. The tree takes its *cut positions* from perceptual geometry:

- `Midpoint`: cut the axis whose longest edge has the longest Oklab chord, at the RGB midpoint.
- `Balanced`: same axis, but cut where that edge's Oklab chord is halved.

The rule is a pure function of the cell, so encoder and decoder rebuild the same tree and nothing
is stored. Any prefix of a code decodes to a coarser cell. With `Length::MaxError` the tree stops
once the cell's estimated worst-case ΔE00 (center-to-corner radius) is under a target. That makes
the code prefix-free and decodable from a concatenated stream (`decode_stream`).

### Why the lattice wins

The lattice keeps the useful half of each earlier idea. Like the tree it spends no codes outside
the gamut, and like Oklab quantization its cells are compact and close to isotropic in a
perceptual space. In-gamut points are found exactly. Along a line of constant `L` and `a`, every
linear RGB channel is a cubic polynomial in `b`, so the in-gamut set of each lattice column is a
union of intervals between polynomial roots. That also covers the hue lines that leave the gamut
and come back in, the case that broke the adaptive spiral.

BCC is the best covering lattice in 3D: for a given point count it has the smallest worst-case
distance to the nearest point. Weighting `L` by `w = 0.4` (coarser lightness) matches CIEDE2000
better than raw Oklab does (see the weight sweep below).

## Method

- **Metrics.** CIEDE2000 (ΔE00) is the main score; the implementation passes Sharma et al.'s
  reference pairs. ΔE_OK is also computed and written to the CSV. The often-quoted
  "ΔE_OK ≈ 0.02 is a JND" has no peer-reviewed source I could find, while ΔE00 ≈ 1 is the
  conventional JND, with measured perceptibility thresholds around 0.8–1.7. Oklab is used as a
  geometry, not as a threshold.
- **Datasets** (100,000 colors each, fixed seed): uniform in sRGB; uniform in Oklab volume within
  the gamut; dark (`L < 0.3`, Oklab-uniform); gamut surface (one channel at 0 or 1); grays.
- **Fairness.** RGB, Oklab-grid and spiral bit splits are picked per budget by p99 ΔE00 on a
  separate calibration set. The lattice weight `w = 0.4` was picked from a single 20-bit sweep.
- Everything is `f64` reference code. Results live in `results/`; regenerate with
  `cargo run --release --bin study -- {fixed|var|prog|speed|lattice}`.

## Results

### Fixed rate (p99 / max ΔE00, uniform-Oklab set)

| bits | RGB (best split) | Oklab grid | Spiral | kd-ok-bal | lattice-cubic w=0.4 | **lattice-bcc w=0.4** |
|---:|---:|---:|---:|---:|---:|---:|
| 16 | 2.70 / 5.77 | 2.54 / 3.77 | 3.08 / 5.18 | 2.29 / 4.78 | 1.48 / 2.64 | **1.41** / 2.85 |
| 20 | 1.14 / 2.34 | 0.97 / 1.36 | 1.15 / 1.79 | 0.91 / 1.86 | 0.58 / 0.85 | **0.55** / 0.86 |
| 24 | 0.45 / 0.93 | 0.41 / 0.60 | 0.48 / 0.65 | 0.38 / 0.74 | 0.23 / 0.40 | **0.22** / **0.34** |
| 28 | 0.16 / 0.38 | 0.16 / 0.24 | 0.19 / 0.33 | 0.14 / 0.27 | 0.09 / 0.14 | **0.09** / 0.14 |
| 32 | 0.069 / 0.139 | 0.061 / 0.088 | 0.072 / 0.115 | 0.057 / 0.107 | 0.036 / 0.054 | **0.034** / 0.056 |

All five datasets are in `results/fixed.md`. The uniform-sRGB, dark and surface sets give the same
ranking. Grays are the exception: RGB keeps grays exactly neutral, while the tree and the lattice
put gray inputs into cells whose centers carry a little chroma, and ΔE00 is very sensitive to
chroma near neutral. At 24 bits, gray p99 is 0.18 for RGB, 0.18 for the BCC lattice and 0.67 for
the tree.

### Bits needed to guarantee a worst case (observed over 100k colors)

| target | dataset | RGB | Oklab grid | lattice-bcc w=0.4 | kd-ok-bal variable (mean / max) |
|---|---|---:|---:|---:|---:|
| ΔE00 ≤ 2 | uniform-sRGB | 21 | 20 | 17 | 15.7 / 21 |
| ΔE00 ≤ 1 | uniform-sRGB | 24 | 22 | **20** | **18.6** / 24 |
| ΔE00 ≤ 1 | dark | 25 | 22 | 20 | 19.5 / 24 |
| ΔE00 ≤ 1 | gray | 18 | 18 | 20 | 23.1 / 24 |
| ΔE00 ≤ 0.5 | uniform-sRGB | 26 | 25 | 23 | 21.6 / 27 |

The variable-length tree's own error bound (its corner-radius estimate) held for every color:
0 of 1.5M samples went over the target.

### Progressive decoding (tree, 32-bit codes)

| codec | p99 ΔE00 after 12 / 16 / 20 / 24 bits | refinements that make a color worse |
|---|---:|---:|
| kd-rgb (plain RGB halving) | 7.13 / 3.43 / 1.18 / 0.45 | 29.2% |
| kd-ok-bal | 6.18 / 2.30 / 0.92 / 0.38 | 28.1% |

Every extra bit shrinks the cell, but about 28% of single-bit refinements move the
reconstruction slightly *away* from the true color. Only the uncertainty bound shrinks
monotonically. That follows from midpoint reconstruction and is not specific to this tree.

### Speed (single thread, unoptimized)

| codec | encode | decode | tables |
|---|---:|---:|---:|
| RGB 11/11/10 | 25 ns | 5 ns | none |
| lattice-bcc 24-bit | 168 ns | 209 ns | 2.5 MiB |
| lattice-bcc 32-bit | 442 ns | 1.3 µs | 101 MiB |
| kd-ok-mid 32-bit | 15 µs | 15 µs | none |
| kd-ok-bal 32-bit | 71 µs | 73 µs | none |

The balanced tree pays for a 16-step bisection at every node. The lattice tables are plain
run-length columns and could be packed far tighter.

## Findings, including the negative ones

1. **Two coordinates was never the constraint; bits are.** Any finite code is a single integer. The
   spiral `(L, t)` works, but at every rate it is worse than plain Oklab quantization.
2. **Wasted codes matter most.** The Oklab grid spends 79% of its codes outside the gamut. The
   lattice's main gain over it is getting those 2.27 bits back.
3. **Cell shape matters next.** The tree's cuts are aligned with the RGB axes, and those axes are
   oblique to perceptual ones, so its cells come out sheared in Oklab. Lattice cells are compact.
   Reweighting the tree's geometry (`okw`) did not close the gap.
4. **Fixing near-black didn't help the tree.** Ottosson's `L_r` toe and CIELAB as alternative split
   geometries were both *worse* than plain Oklab under ΔE00, dark colors included
   (`results/fixed_geometries.md`).
5. **Raw Oklab over-weights lightness compared to CIEDE2000.** Lattice p99 improves steadily from
   `w = 1` down to `w ≈ 0.4` (`results/lattice_sweep.md`).
6. **BCC beats cubic by less than theory predicts.** Under ΔE_OK at `w = 1`, BCC lowers p99 by
   about 13% but the max by only 3–9%, far from the 23% the covering radius predicts
   (`cargo run --release --example bcc_vs_cubic`). Colors near the gamut boundary set the max,
   because their nearest *in-gamut* point is farther away. Under ΔE00 the mismatch between the
   two geometries shrinks the gain further.
7. **Grays are the weak spot** for both new codecs (see above).

## Flattening the gamut into 2D ("unfolding")

**Question.** Can the 3D gamut be unfolded onto a plane like a cube net, so a color becomes two
coordinates, and does that buy fewer bits, better image compression, or a usable 2D color map?

**Theory first.**
- A cube's *surface* is 2D, so it unfolds. The gamut is a *volume*, and no continuous injection
  R³ → R² exists (invariance of domain). Every 2D flattening must tear the volume somewhere.
- Borsuk–Ulam adds that any continuous map of a sphere-like shell into the plane merges some pair
  of antipodal colors.
- Space-filling curves are at best 1/d-Hölder.

**Per-color bits are not the lever.**
- Quantized, any point set fits any number of coordinates; the lattice rank is already one.
- A 2D layout of the same lattice points needs exactly the same `log2(N)` bits, so ~20 bits per
  color at ΔE00 ≤ 1 regardless of shape.
- What a layout *can* change is locality.

**What was built** (`src/flat/`): bijective layouts of the 20-bit (1024²) and 16-bit (256²) BCC
lattices. A method gives each point a 2D target position; recursive median bisection then fills
the grid evenly.

| layout | idea |
|---|---|
| raster | rank in row-major order (control) |
| Morton / Hilbert 3D→2D | sort points along a 3D curve, lay the sequence along the 2D curve |
| L-slices | 3D-LUT atlas: lightness slabs as tiles, (a, b) inside |
| Okhsv slices | Ottosson's exact gamut→cylinder; value slabs as tiles, hue/saturation disk inside |
| shells-octa | onion shells around the gamut's center (the gamut is star-shaped there: 0 points beyond a ray's first exit), each unfolded with the equal-area octahedral map |
| onion rings | shells as annuli of one disk; elevation alternates direction per shell so neighboring shells meet at the same pole |
| kernel PCA / Isomap | RBF kernel PCA and landmark Isomap embeddings (1500 landmarks) |

"Mirrored" means every other tile is flipped, so that touching edges of consecutive tiles show
matching colors. This is the cube-net folding trick.

### Results

**Locality** (`results/flat_layouts.md`): adjacent cells and "near-twin" colors one lattice step
apart.

| layout (20-bit) | adjacent-cell ΔE00 mean / p99 | visible seams (ΔE00 > 5) | near twins > 4 cells apart |
|---|---:|---:|---:|
| raster | 13.5 / 75.7 | 48.9% | 78% |
| Hilbert 3D→2D | 1.48 / 12.4 | 3.8% | 43% |
| L-slices 16 | 1.83 / 8.0 | 2.9% | 26% |
| **L-slices 64 mirrored** | **0.85 / 3.8** | **0.66%** | 35% |
| Okhsv slices 64 mirrored | 0.98 / 6.0 | 1.3% | 45% |
| shells-octa 16 mirrored | 1.20 / 8.5 | 2.6% | 39% |
| onion rings 8 | 2.67 / 18.4 | 13% | 65% |

At 16 bits, trustworthiness / continuity (Venna & Kaski, k = 10):

| 16-bit layout | trustworthiness / continuity |
|---|---:|
| L-slices mirrored | 0.998 / 0.955 |
| Hilbert | 0.984 / 0.959 |
| kernel PCA | 0.896 / 0.998 |
| Isomap | 0.907 / 0.997 |

The kernel methods keep color neighbors together (continuity) but pile unrelated colors next to
each other (trustworthiness). They squash the lightness dimension into speckle, and 67–77% of
their adjacent cells are visible seams.

**Images** (`results/flat_images.md`): 13 wallpapers at 1024 px wide, quantized to the 20-bit
lattice (p99 ΔE00 0.2–0.7). Mean bits per pixel:

| encoding of the *same* quantized image | bpp |
|---|---:|
| RGB888 → WebP lossless | **3.36** |
| RGB888 → Paeth + zstd (same coder as the layouts) | 4.80 |
| 1-D lattice rank → zstd | 5.31 |
| best 2D layout (Okhsv slices mirrored) → Paeth + zstd | 5.95 |
| L-slices mirrored → Paeth + zstd | 6.03 |
| Hilbert 3D→2D → Paeth + zstd | 6.39 |
| raster → Paeth + zstd | 7.92 |

For reference, the zero-order entropy of the ranks is 7.21 bpp, and the 8-bit originals take
3.40 bpp in WebP lossless.

**Interpolating codes:** averaging the codes of neighboring pixels gives a color more than ΔE00 2
off the true blend this often:

| codes averaged | pairs off by ΔE00 > 2 |
|---|---:|
| RGB888 | 0% |
| Hilbert 3D→2D | 41% |
| L-slices | 46% |
| onion rings | 70% |

### Verdict

1. **Fewer bits: no.** The per-color budget is set by how many colors must be distinguished, not
   by how many coordinates carry them. On images, every 2D layout costs **24% more** bits than plain
   RGB888 under the same coder, and more than the 1-D rank.
2. **Why RGB wins on images: continuity beats coordinate count.** Natural images vary smoothly,
   and RGB, like any continuous 3-coordinate system, keeps gradients smooth in every channel. A 2D
   layout must tear, and each tear a gradient crosses becomes an expensive residual. Squeezing
   out a dimension trades one coordinate for many discontinuities.
3. **Maps: yes, and folding helps.** The mirrored L-slice atlas is a genuinely good 2D map of the
   whole gamut. Each of the 2²⁰ in-gamut lattice colors appears exactly once, and only 0.66% of
   adjacent cells form a visible seam. Mirroring cut the p99 seam in the 16-bit map from 43 to
   9.9 ΔE00. See `results/maps/` (256² renders) and the overviews
   `results/maps/overview-tiled.png` and `results/maps/overview-curves-rings-kernel.png`.
4. **Kernel and manifold methods can't flatten a solid.** They recover hue and chroma and crush
   lightness, exactly as invariance of domain predicts.
5. **Interpolation needs continuity.** Even the best layouts give a wrong blend for 40–70% of
   pairs, so filtering and blending must happen after decoding.

Run with `cargo run --release --bin flat -- {layouts|images}`. The image set is whatever is in
`/usr/share/backgrounds`; WebP numbers need ImageMagick with libwebp.

## Prior art

- **Fibonacci-lattice color quantization**, Mojsilović & Soljanin (SPIE 2000, IEEE TIP 2001): a
  fixed, image-independent codebook from spiral lattices in Lab. It is the closest prior work to a
  universal perceptual color code, and to the spiral idea. It is not binary or progressive.
- **Tree-structured and entropy-constrained VQ** (Chou, Lookabaugh & Gray 1989; Riskin & Gray
  1991), plus median cut, octree, Wu, and Orchard–Bouman for palettes. All of these are
  image-adaptive. I found no fixed, Oklab-guided k-d partition of the whole sRGB gamut used as a
  progressive universal color code, which is absence of evidence, not proof.
- **Tree-VQ** (arXiv:2609.03641, Sep 2026): progressive prefix-decodable trees over learned image
  tokens, not single colors.
- **JNDs**: ITU-R BT.2124 (ΔE_ITP = 1 ≈ JND); Dolby PQ/ICtCp bit-depth work (10-bit ICtCp ≈
  11.5-bit YCbCr); Ottosson's Oklab and colorpicker posts (the `L_r` toe).

## Limitations

- Synthetic datasets only. No natural images yet, and no entropy coding of image streams.
- ΔE00 stands in for perception. No viewing study was run.
- "Bits needed" counts are observed maxima over 100k samples, not proofs (except the tree's own
  corner-radius bound, which is also an estimate but held empirically).
- `w = 0.4` was tuned once, at 20 bits, on the evaluation seed. The margins it produces are far
  larger than its sensitivity, but a clean held-out tuning is still to do.

## Next steps

- **Progressive lattice:** order lattice ranks hierarchically, for example a gamut-only binary
  partition of the lattice point set. That would combine the lattice's quality with the tree's
  prefix decoding and variable length.
- **Neutral axis:** put a lattice coset on the gray line, or snap near-neutral reconstructions,
  to remove the gray penalty.
- **Smaller tables:** one `(k0, k1, rank)` per column plus a rare-exception list instead of
  generic runs; or compute columns on the fly with the cubic-root solver and cache them.
- **Images:** entropy-code lattice ranks or tree paths over real photos and compare with RGB888,
  WebP lossless and RGB9E5/R11G11B10F.

## Layout

```
src/color.rs            sRGB / linear / Oklab / CIELAB conversions (f64)
src/metric.rs           ΔE_OK, CIEDE2000 (Sharma reference tests)
src/code.rs             Code (≤64-bit MSB-first bit string), Codec trait
src/codecs/baseline.rs  RGB, Oklab-grid and spiral codecs
src/codecs/kd.rs        perceptual binary partition (fixed / variable length, streams)
src/codecs/lattice.rs   gamut-ranked cubic / BCC lattices with exact cubic-root columns
src/data.rs             seeded test color sets
src/stats.rs            summaries, parallel evaluation
src/flat/               2D layouts ("unfolding"): Hilbert/Morton, slice atlases, Okhsv, octahedral
                        shells, onion rings, kernel PCA / Isomap, locality metrics, plane coding
src/bin/study.rs        codec experiments; writes markdown and results/fixed.csv
src/bin/flat.rs         flattening experiments; writes results/flat_*.md and results/maps/
examples/gamut_fraction.rs  in-gamut share of the Oklab bounding box
```
