//! Okhsv (Björn Ottosson, https://bottosson.github.io/posts/colorpicker/), ported to f64.
//!
//! Okhsv maps the sRGB gamut exactly onto the cylinder `h ∈ [0,1), s ∈ [0,1], v ∈ [0,1]`, which
//! makes it the natural "gamut as a box" coordinate system for slicing and tiling.

use crate::codecs::kd::toe;
use crate::color::{Srgb, linear_to_oklab, oklab_to_linear, srgb_to_linear};

const K1: f64 = 0.206;
const K2: f64 = 0.03;
const K3: f64 = (1.0 + K1) / (1.0 + K2);

fn toe_inv(x: f64) -> f64 {
    (x * x + K1 * x) / (K3 * (x + K2))
}

/// Maximum saturation `S = C/L` for a normalized hue direction `(a, b)` (polynomial fit plus one
/// Halley step, as in the reference implementation).
fn compute_max_saturation(a: f64, b: f64) -> f64 {
    let (k0, k1, k2, k3, k4, wl, wm, ws);
    if -1.881_703_28 * a - 0.809_364_93 * b > 1.0 {
        // Red component goes below zero first.
        (k0, k1, k2, k3, k4) = (1.190_862_77, 1.765_767_28, 0.596_626_41, 0.755_151_97, 0.567_712_45);
        (wl, wm, ws) = (4.076_741_662_1, -3.307_711_591_3, 0.230_969_929_2);
    } else if 1.814_441_04 * a - 1.194_452_76 * b > 1.0 {
        // Green.
        (k0, k1, k2, k3, k4) = (0.739_565_15, -0.459_544_04, 0.082_854_27, 0.125_410_70, 0.145_032_04);
        (wl, wm, ws) = (-1.268_438_004_6, 2.609_757_401_1, -0.341_319_396_5);
    } else {
        // Blue.
        (k0, k1, k2, k3, k4) = (1.357_336_52, -0.009_157_99, -1.151_302_10, -0.505_596_06, 0.006_921_67);
        (wl, wm, ws) = (-0.004_196_086_3, -0.703_418_614_7, 1.707_614_701_0);
    }
    let mut s = k0 + k1 * a + k2 * b + k3 * a * a + k4 * a * b;
    let k_l = 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let k_m = -0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let k_s = -0.089_484_177_5 * a - 1.291_485_548_0 * b;
    // The reference does one Halley step (f32-accurate to ~1e-3); iterate to convergence.
    for _ in 0..4 {
        let (l_, m_, s_) = (1.0 + s * k_l, 1.0 + s * k_m, 1.0 + s * k_s);
        let (l, m, ss) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        let (l_ds, m_ds, s_ds) = (3.0 * k_l * l_ * l_, 3.0 * k_m * m_ * m_, 3.0 * k_s * s_ * s_);
        let (l_ds2, m_ds2, s_ds2) = (6.0 * k_l * k_l * l_, 6.0 * k_m * k_m * m_, 6.0 * k_s * k_s * s_);
        let f = wl * l + wm * m + ws * ss;
        let f1 = wl * l_ds + wm * m_ds + ws * s_ds;
        let f2 = wl * l_ds2 + wm * m_ds2 + ws * s_ds2;
        s -= f * f1 / (f1 * f1 - 0.5 * f * f2);
    }
    s
}

/// `(L, C)` of the most saturated in-gamut color of a hue.
fn find_cusp(a: f64, b: f64) -> (f64, f64) {
    let s_cusp = compute_max_saturation(a, b);
    let rgb = oklab_to_linear([1.0, s_cusp * a, s_cusp * b]);
    let l_cusp = (1.0 / rgb[0].max(rgb[1]).max(rgb[2])).cbrt();
    (l_cusp, l_cusp * s_cusp)
}

/// sRGB to Okhsv `(h, s, v)`.
pub fn srgb_to_okhsv(c: Srgb) -> [f64; 3] {
    let [l, a, b] = linear_to_oklab(srgb_to_linear(c));
    let chroma = a.hypot(b);
    let (a_, b_) = if chroma > 1e-12 { (a / chroma, b / chroma) } else { (1.0, 0.0) };
    if l <= 1e-12 {
        return [0.0, 0.0, 0.0];
    }
    let h = 0.5 + 0.5 * (-b).atan2(-a) / std::f64::consts::PI;
    let (cusp_l, cusp_c) = find_cusp(a_, b_);
    let (s_max, t_max) = (cusp_c / cusp_l, cusp_c / (1.0 - cusp_l));
    let s0 = 0.5;
    let k = 1.0 - s0 / s_max;
    // Invert the triangle mapping and the toe.
    let t = t_max / (chroma + l * t_max);
    let l_v = t * l;
    let c_v = t * chroma;
    let l_vt = toe_inv(l_v);
    let c_vt = c_v * l_vt / l_v;
    let rgb_scale = oklab_to_linear([l_vt, a_ * c_vt, b_ * c_vt]);
    let scale_l = (1.0 / rgb_scale[0].max(rgb_scale[1]).max(rgb_scale[2]).max(0.0)).cbrt();
    let v = toe(l / scale_l) / l_v;
    let s = (s0 + t_max) * c_v / (t_max * s0 + t_max * k * c_v);
    [h, s, v]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primaries_and_neutrals() {
        let red = srgb_to_okhsv([1.0, 0.0, 0.0]);
        assert!((red[1] - 1.0).abs() < 1e-3 && (red[2] - 1.0).abs() < 1e-3, "{red:?}");
        let blue = srgb_to_okhsv([0.0, 0.0, 1.0]);
        assert!((blue[1] - 1.0).abs() < 1e-3 && (blue[2] - 1.0).abs() < 1e-3, "{blue:?}");
        let white = srgb_to_okhsv([1.0, 1.0, 1.0]);
        assert!(white[1].abs() < 1e-6 && (white[2] - 1.0).abs() < 1e-6, "{white:?}");
        let gray = srgb_to_okhsv([0.5, 0.5, 0.5]);
        assert!(gray[1].abs() < 1e-6 && gray[2] > 0.3 && gray[2] < 0.7, "{gray:?}");
    }

    #[test]
    fn gamut_lands_in_unit_cylinder() {
        let mut rng = crate::data::Rng::new(11);
        for _ in 0..50_000 {
            let c = [rng.f64(), rng.f64(), rng.f64()];
            let [h, s, v] = srgb_to_okhsv(c);
            assert!((0.0..=1.0).contains(&h), "{c:?} -> h {h}");
            assert!((-1e-6..=1.0 + 1e-6).contains(&s), "{c:?} -> s {s}");
            assert!((-1e-6..=1.0 + 1e-6).contains(&v), "{c:?} -> v {v}");
        }
    }
}
