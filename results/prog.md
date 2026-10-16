## Progressive refinement

Uniform-Oklab set. Share of one-bit refinements (prefix n → n+1, n ≥ 8) that *increase* ΔE00, and p99 ΔE00 by prefix length.

| codec | worse refinements | p99 @8 | @12 | @16 | @20 | @24 | @28 |
|---|---:|---:|---:|---:|---:|---:|---:|
| kd-rgb 32b | 29.2% | 18.44 | 7.13 | 3.43 | 1.182 | 0.451 | 0.212 |
| kd-ok-bal 32b | 28.1% | 14.88 | 6.18 | 2.30 | 0.918 | 0.379 | 0.144 |
| kd-okt-bal 32b | 28.3% | 16.00 | 6.34 | 2.35 | 0.980 | 0.391 | 0.149 |
| kd-lab-bal 32b | 28.3% | 15.97 | 6.72 | 2.53 | 1.063 | 0.422 | 0.160 |

