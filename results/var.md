## Bits needed for a worst-case CIEDE2000 target

kd rows: variable-length tree that stops once the cell's corner radius in ΔE00 is ≤ target (mean and max code length, observed error). Fixed-rate rows: smallest budget whose *observed* max ΔE00 on the same 100000 colors meets the target (RGB/Oklab: best bit split).

### max ΔE00 ≤ 2

| dataset | kd-ok-bal mean bits | kd max bits | kd obs. max ΔE00 | kd over target | RGB bits | Oklab-grid bits | lattice-bcc w=0.4 bits |
|---|---:|---:|---:|---:|---:|---:|---:|
| uniform-sRGB | 15.74 | 21 | 1.884 | 0.000% | 21 | 20 | 17 |
| uniform-Oklab | 16.07 | 21 | 1.862 | 0.000% | 21 | 19 | 17 |
| dark (L<0.3) | 16.57 | 21 | 1.866 | 0.000% | 22 | 19 | 17 |
| gamut surface | 15.23 | 20 | 1.960 | 0.000% | 20 | 20 | 18 |
| gray | 20.08 | 21 | 1.879 | 0.000% | 15 | 16 | 15 |

### max ΔE00 ≤ 1

| dataset | kd-ok-bal mean bits | kd max bits | kd obs. max ΔE00 | kd over target | RGB bits | Oklab-grid bits | lattice-bcc w=0.4 bits |
|---|---:|---:|---:|---:|---:|---:|---:|
| uniform-sRGB | 18.64 | 24 | 0.939 | 0.000% | 24 | 22 | 20 |
| uniform-Oklab | 18.99 | 24 | 0.953 | 0.000% | 24 | 22 | 20 |
| dark (L<0.3) | 19.46 | 24 | 0.949 | 0.000% | 25 | 22 | 20 |
| gamut surface | 18.08 | 23 | 0.979 | 0.000% | 23 | 23 | 21 |
| gray | 23.12 | 24 | 0.889 | 0.000% | 18 | 18 | 20 |

### max ΔE00 ≤ 0.5

| dataset | kd-ok-bal mean bits | kd max bits | kd obs. max ΔE00 | kd over target | RGB bits | Oklab-grid bits | lattice-bcc w=0.4 bits |
|---|---:|---:|---:|---:|---:|---:|---:|
| uniform-sRGB | 21.59 | 27 | 0.480 | 0.000% | 26 | 25 | 23 |
| uniform-Oklab | 21.95 | 27 | 0.467 | 0.000% | 27 | 25 | 23 |
| dark (L<0.3) | 22.40 | 27 | 0.483 | 0.000% | 27 | 25 | 23 |
| gamut surface | 21.00 | 26 | 0.490 | 0.000% | 25 | 26 | 24 |
| gray | 26.14 | 27 | 0.443 | 0.000% | 21 | 20 | 21 |

