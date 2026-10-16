//! Color difference metrics.

use crate::color::{Srgb, linear_to_lab, srgb_to_linear, srgb_to_oklab};

/// Euclidean distance in Oklab between two sRGB colors (ΔE_OK).
#[inline]
pub fn delta_e_ok(x: Srgb, y: Srgb) -> f64 {
    dist(srgb_to_oklab(x), srgb_to_oklab(y))
}

#[inline]
pub fn dist(x: [f64; 3], y: [f64; 3]) -> f64 {
    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

/// CIEDE2000 between two sRGB colors (D65 CIELAB).
pub fn delta_e_2000_srgb(x: Srgb, y: Srgb) -> f64 {
    delta_e_2000(linear_to_lab(srgb_to_linear(x)), linear_to_lab(srgb_to_linear(y)))
}

/// CIEDE2000 color difference (Sharma, Wu & Dalal, 2005), with `kL = kC = kH = 1`.
pub fn delta_e_2000([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    const P25_7: f64 = 6_103_515_625.0; // 25^7
    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let cbar7 = ((c1 + c2) / 2.0).powi(7);
    let g = 0.5 * (1.0 - (cbar7 / (cbar7 + P25_7)).sqrt());
    let a1p = (1.0 + g) * a1;
    let a2p = (1.0 + g) * a2;
    let c1p = a1p.hypot(b1);
    let c2p = a2p.hypot(b2);
    let hue = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let h1p = hue(b1, a1p);
    let h2p = hue(b2, a2p);

    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else {
        let d = h2p - h1p;
        if d > 180.0 {
            d - 360.0
        } else if d < -180.0 {
            d + 360.0
        } else {
            d
        }
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh.to_radians() / 2.0).sin();

    let lbar = (l1 + l2) / 2.0;
    let cbarp = (c1p + c2p) / 2.0;
    let hbar = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hbar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hbar).to_radians().cos()
        + 0.32 * (3.0 * hbar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hbar - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hbar - 275.0) / 25.0).powi(2)).exp();
    let cbarp7 = cbarp.powi(7);
    let rc = 2.0 * (cbarp7 / (cbarp7 + P25_7)).sqrt();
    let lb50 = (lbar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * lb50 / (20.0 + lb50).sqrt();
    let sc = 1.0 + 0.045 * cbarp;
    let sh = 1.0 + 0.015 * cbarp * t;
    let rt = -(2.0 * dtheta).to_radians().sin() * rc;
    let (x, y, z) = (dl / sl, dc / sc, dhh / sh);
    (x * x + y * y + z * z + rt * y * z).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference pairs from Sharma, Wu & Dalal (2005), Table 1.
    #[test]
    fn ciede2000_sharma_reference_pairs() {
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
            ([50.0, 2.8361, -74.0200], [50.0, 0.0, -82.7485], 3.4412),
            ([50.0, -1.3802, -84.2814], [50.0, 0.0, -82.7485], 1.0000),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            ([2.0776, 0.0795, -1.1350], [0.9033, -0.0636, -0.5514], 0.9082),
        ];
        for (x, y, want) in cases {
            let got = delta_e_2000(x, y);
            assert!((got - want).abs() < 1e-4, "{x:?} {y:?}: got {got}, want {want}");
        }
    }
}
