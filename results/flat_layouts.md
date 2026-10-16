## 16-bit BCC lattice (w = 0.4): 65523 points on a 256×256 grid

Shell center (Oklab mean of the gamut points): L=0.594 a=0.032 b=-0.023. Points beyond the first gamut exit of their ray (gamut not star-shaped there): 0 (0.000%).

| layout | adjacent-cell ΔE00 mean / p99 | seams ΔE00>5 | near-twin distance median / p99 (cells) | twins >4 cells apart | trust. / cont. (k=10) |
|---|---:|---:|---:|---:|---:|
| raster | 16.09 / 75.4 | 51.60% | 130.5 / 258 | 77.8% | 0.7545 / 0.4682 |
| morton-3d→2d | 4.50 / 35.3 | 21.66% | 4.1 / 201 | 50.8% | 0.9730 / 0.9414 |
| hilbert-3d→2d | 3.57 / 29.5 | 14.90% | 3.0 / 166 | 41.9% | 0.9841 / 0.9589 |
| L-slices 16 | 2.88 / 43.2 | 4.95% | 2.0 / 71 | 25.8% | 0.9810 / 0.9557 |
| L-slices 16 mirrored | 2.39 / 9.9 | 4.44% | 2.0 / 120 | 25.4% | 0.9984 / 0.9554 |
| L-slices 64 mirrored | 2.58 / 17.9 | 6.50% | 15.1 / 107 | 57.7% | 0.9949 / 0.9249 |
| okhsv-slices 16 mirrored | 2.54 / 14.5 | 6.92% | 2.2 / 123 | 33.0% | 0.9983 / 0.9366 |
| okhsv-slices 64 mirrored | 3.01 / 12.0 | 12.54% | 26.4 / 128 | 62.7% | 0.9974 / 0.8957 |
| shells-octa 16 mirrored | 3.08 / 13.0 | 14.59% | 13.2 / 158 | 55.6% | 0.9973 / 0.8398 |
| onion-rings 8 pole=L | 6.34 / 32.3 | 41.23% | 5.7 / 48 | 60.5% | 0.9705 / 0.9899 |
| onion-rings 32 pole=L | 14.37 / 66.4 | 71.07% | 7.0 / 33 | 71.4% | 0.8767 / 0.9940 |
| onion-rings 16 pole=a | 9.29 / 57.0 | 51.57% | 7.1 / 37 | 67.8% | 0.9226 / 0.9937 |
| kernel-PCA (RBF, 1500 landmarks) | 15.72 / 58.4 | 77.45% | 5.0 / 18 | 59.2% | 0.8957 / 0.9982 |
| isomap (1500 landmarks, k=10) | 14.21 / 58.0 | 67.13% | 4.5 / 26 | 57.4% | 0.9074 / 0.9969 |

(near-twin pairs: mean ΔE00 between the two decoded colors ≈ 1.48; built and measured in 16s)

## 20-bit BCC lattice (w = 0.4): 1048089 points on a 1024×1024 grid

Shell center (Oklab mean of the gamut points): L=0.594 a=0.032 b=-0.023. Points beyond the first gamut exit of their ray (gamut not star-shaped there): 0 (0.000%).

| layout | adjacent-cell ΔE00 mean / p99 | seams ΔE00>5 | near-twin distance median / p99 (cells) | twins >4 cells apart |
|---|---:|---:|---:|---:|
| raster | 13.54 / 75.7 | 48.89% | 519.1 / 1028 | 78.1% |
| morton-3d→2d | 1.91 / 15.9 | 5.91% | 4.1 / 602 | 51.0% |
| hilbert-3d→2d | 1.48 / 12.4 | 3.83% | 3.0 / 482 | 42.9% |
| L-slices 16 | 1.83 / 8.0 | 2.87% | 2.8 / 269 | 25.9% |
| L-slices 16 mirrored | 1.70 / 7.5 | 2.77% | 2.2 / 456 | 26.0% |
| L-slices 64 mirrored | 0.85 / 3.8 | 0.66% | 2.0 / 250 | 35.3% |
| okhsv-slices 16 mirrored | 1.71 / 13.3 | 4.94% | 2.2 / 467 | 25.0% |
| okhsv-slices 64 mirrored | 0.98 / 6.0 | 1.28% | 2.2 / 257 | 45.1% |
| shells-octa 16 mirrored | 1.20 / 8.5 | 2.56% | 2.8 / 498 | 39.1% |
| onion-rings 8 pole=L | 2.67 / 18.4 | 13.05% | 6.7 / 137 | 64.5% |
| onion-rings 32 pole=L | 5.81 / 31.0 | 38.15% | 10.0 / 75 | 74.3% |
| onion-rings 16 pole=a | 3.41 / 22.9 | 19.93% | 8.6 / 93 | 69.7% |

(near-twin pairs: mean ΔE00 between the two decoded colors ≈ 0.58; built and measured in 5s)

