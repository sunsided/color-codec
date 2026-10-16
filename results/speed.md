## Speed (single thread, unoptimized f64 reference code)

| codec | encode ns/color | decode ns/color |
|---|---:|---:|
| RGB 11/11/10 | 25 | 5 |
| Oklab 11/11/10 | 131 | 60 |
| Spiral 11/21 | 522 | 85 |
| kd-ok-mid 32b | 14827 | 14545 |
| kd-ok-bal 32b | 70994 | 72674 |
| kd-ok-bal ΔE00≤1 | 82730 | 41635 |
| lattice-bcc w=0.4 24b | 168 | 209 |
| lattice-bcc w=0.4 32b | 442 | 1339 |

- lattice-bcc w=0.4 16b: 65523 points, rank tables 0.1 MiB
- lattice-bcc w=0.4 20b: 1048089 points, rank tables 0.4 MiB
- lattice-bcc w=0.4 24b: 16768829 points, rank tables 2.5 MiB
- lattice-bcc w=0.4 28b: 268301267 points, rank tables 15.9 MiB
- lattice-bcc w=0.4 32b: 4292819830 points, rank tables 100.6 MiB

