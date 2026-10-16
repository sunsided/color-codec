//! Color space conversions: sRGB (gamma-encoded) <-> linear sRGB <-> Oklab, plus CIELAB (D65).
//!
//! All math is done in `f64` so that conversion noise stays far below the quantization error
//! of any codec under test.

/// Gamma-encoded sRGB triplet, nominally in `[0, 1]`.
pub type Srgb = [f64; 3];
/// Linear-light sRGB triplet.
pub type LinRgb = [f64; 3];
/// Oklab `(L, a, b)`.
pub type Oklab = [f64; 3];

#[inline]
pub fn srgb_to_linear_1(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn linear_to_srgb_1(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
pub fn srgb_to_linear(c: Srgb) -> LinRgb {
    c.map(srgb_to_linear_1)
}

#[inline]
pub fn linear_to_srgb(c: LinRgb) -> Srgb {
    c.map(linear_to_srgb_1)
}

/// Linear sRGB to Oklab (Björn Ottosson, 2020).
#[inline]
pub fn linear_to_oklab([r, g, b]: LinRgb) -> Oklab {
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// Oklab to linear sRGB. The result may fall outside `[0, 1]` for out-of-gamut colors.
#[inline]
pub fn oklab_to_linear([ll, a, b]: Oklab) -> LinRgb {
    let l = ll + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m = ll - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s = ll - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
    ]
}

#[inline]
pub fn srgb_to_oklab(c: Srgb) -> Oklab {
    linear_to_oklab(srgb_to_linear(c))
}

/// Oklab to gamma-encoded sRGB, clipping out-of-gamut values per channel in linear light.
#[inline]
pub fn oklab_to_srgb_clipped(c: Oklab) -> Srgb {
    linear_to_srgb(oklab_to_linear(c).map(|v| v.clamp(0.0, 1.0)))
}

/// Clamp each channel into `[0, 1]`.
#[inline]
pub fn clamp01(c: [f64; 3]) -> [f64; 3] {
    c.map(|v| v.clamp(0.0, 1.0))
}

/// Linear sRGB to CIELAB with a D65 white point.
pub fn linear_to_lab([r, g, b]: LinRgb) -> [f64; 3] {
    const XN: f64 = 0.950_47;
    const YN: f64 = 1.0;
    const ZN: f64 = 1.088_83;
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    fn f(t: f64) -> f64 {
        const D: f64 = 6.0 / 29.0;
        if t > D * D * D {
            t.cbrt()
        } else {
            t / (3.0 * D * D) + 4.0 / 29.0
        }
    }
    let (fx, fy, fz) = (f(x / XN), f(y / YN), f(z / ZN));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_and_black_map_to_oklab_extremes() {
        let w = srgb_to_oklab([1.0, 1.0, 1.0]);
        assert!((w[0] - 1.0).abs() < 1e-6 && w[1].abs() < 1e-6 && w[2].abs() < 1e-6);
        let k = srgb_to_oklab([0.0, 0.0, 0.0]);
        assert!(k.iter().all(|v| v.abs() < 1e-12));
    }

    #[test]
    fn oklab_round_trip() {
        for &c in &[[0.1, 0.5, 0.9], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.3, 0.3, 0.3]] {
            let back = oklab_to_srgb_clipped(srgb_to_oklab(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-6, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn srgb_transfer_round_trip() {
        for i in 0..=1000 {
            let v = i as f64 / 1000.0;
            assert!((linear_to_srgb_1(srgb_to_linear_1(v)) - v).abs() < 1e-9);
        }
    }
}
