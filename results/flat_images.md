## Images: bits per pixel

Images from `/usr/share/backgrounds`, resized to 1024 px wide (Lanczos3), quantized to the 20-bit BCC lattice. *orig* columns are the 8-bit original (zero loss); every other column carries the same lattice quantization error (ΔE00 shown). Layout columns: (u, v) planes, Paeth-predicted, byte planes, zstd -19. *quant WebP* is the quantized image as RGB888 in WebP lossless, the standard codec at equal distortion.

| image | px | ΔE00 p99 / max | rank entropy | orig PNG | orig WebP | quant WebP | quant RGB Paeth+zstd | rank zstd | raster | morton-3d→2d | hilbert-3d→2d | L-slices 16 mirrored | okhsv-slices 16 mirrored | shells-octa 16 mirrored | onion-rings 8 pole=L |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Clouds_by_Tibor_Mokanszk | 1024×576 | 0.69 / 1.01 | 11.26 | 7.98 | 6.12 | 7.07 | 8.08 | 9.41 | 13.60 | 11.76 | 10.58 | 9.76 | 9.70 | 10.22 | 11.04 |
| Fuji_san_by_amaral | 1024×576 | 0.72 / 0.88 | 10.35 | 7.76 | 5.98 | 6.77 | 8.11 | 8.96 | 12.96 | 11.61 | 10.78 | 9.94 | 9.98 | 10.02 | 11.40 |
| Fuwafuwa_nanbatto_san_by | 1024×576 | 0.20 / 0.43 | 1.68 | 1.80 | 1.01 | 0.76 | 0.95 | 1.59 | 3.05 | 2.54 | 2.68 | 2.37 | 2.40 | 2.51 | 2.32 |
| Fuwafuwa_nanbatto_san_by | 1024×576 | 0.47 / 0.55 | 7.79 | 3.97 | 2.75 | 2.31 | 3.79 | 3.42 | 5.97 | 5.06 | 4.56 | 4.19 | 4.06 | 4.54 | 4.67 |
| Little_numbat_boy_by_azs | 1024×576 | 0.49 / 0.91 | 11.17 | 9.68 | 6.96 | 7.67 | 10.19 | 11.69 | 15.33 | 13.63 | 12.97 | 11.30 | 10.85 | 11.97 | 13.02 |
| Monument_valley_by_orbit | 1024×574 | 0.55 / 0.86 | 8.41 | 3.96 | 2.85 | 2.84 | 4.01 | 4.51 | 5.72 | 5.08 | 4.87 | 4.48 | 4.36 | 4.63 | 4.88 |
| Numbat_wallpaper_dimmed_ | 1024×576 | 0.68 / 0.74 | 4.52 | 3.03 | 2.23 | 1.38 | 3.37 | 3.28 | 6.28 | 4.98 | 4.58 | 4.73 | 4.74 | 4.96 | 4.70 |
| Numbat_wallpaper_light_3 | 1024×576 | 0.71 / 0.90 | 5.43 | 3.54 | 2.58 | 1.39 | 3.58 | 3.48 | 6.55 | 4.90 | 4.56 | 4.59 | 4.12 | 4.75 | 4.88 |
| Province_of_the_south_of | 1024×576 | 0.60 / 0.85 | 6.47 | 3.68 | 2.52 | 2.53 | 3.80 | 3.92 | 5.30 | 4.71 | 4.45 | 4.20 | 4.05 | 4.31 | 4.46 |
| Rainbow_lightbulb_by_Dan | 1024×683 | 0.58 / 1.09 | 9.90 | 6.57 | 4.82 | 5.44 | 6.74 | 8.64 | 10.35 | 9.85 | 9.45 | 8.83 | 8.98 | 9.13 | 9.66 |
| ubuntu-default-greyscale | 1024×576 | 0.68 / 0.74 | 4.52 | 3.03 | 2.23 | 1.38 | 3.37 | 3.28 | 6.28 | 4.98 | 4.58 | 4.73 | 4.74 | 4.96 | 4.70 |
| ubuntu-wallpaper-d | 1024×576 | 0.71 / 0.74 | 3.75 | 2.12 | 1.44 | 1.04 | 2.23 | 2.44 | 4.41 | 3.67 | 3.35 | 3.65 | 3.85 | 3.45 | 3.62 |
| warty-final-ubuntu | 1024×576 | 0.41 / 0.57 | 8.45 | 3.69 | 2.71 | 3.04 | 4.21 | 4.42 | 7.18 | 6.22 | 5.60 | 5.59 | 5.55 | 5.36 | 6.39 |
| **mean** | | | **7.21** | **4.68** | **3.40** | **3.36** | **4.80** | **5.31** | **7.92** | **6.85** | **6.39** | **6.03** | **5.95** | **6.22** | **6.59** |

### Interpolating codes

Horizontal neighbor pixels with different codes: average their (u, v), decode, compare with the Oklab midpoint of the two colors. An empty cell counts as a failure (ΔE00 = 100).

| layout | pairs | ΔE00 median | p99 | share > 2 |
|---|---:|---:|---:|---:|
| RGB888 (baseline: average the channels) | 342897 | 0.11 | 0.5 | 0.0% |
| raster | 342897 | 36.27 | 72.3 | 92.7% |
| morton-3d→2d | 342897 | 2.31 | 46.4 | 54.8% |
| hilbert-3d→2d | 342897 | 1.58 | 33.1 | 40.8% |
| L-slices 16 mirrored | 342897 | 1.78 | 51.9 | 45.5% |
| okhsv-slices 16 mirrored | 342897 | 2.35 | 73.0 | 53.5% |
| shells-octa 16 mirrored | 342897 | 1.77 | 77.6 | 47.7% |
| onion-rings 8 pole=L | 342897 | 5.74 | 82.4 | 69.6% |

