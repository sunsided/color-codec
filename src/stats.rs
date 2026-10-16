//! Error statistics and a small parallel evaluation helper.

use crate::code::Codec;
use crate::color::Srgb;
use crate::metric::{delta_e_2000_srgb, delta_e_ok};

#[derive(Clone, Copy, Debug, Default)]
pub struct Summary {
    pub mean: f64,
    pub p50: f64,
    pub p99: f64,
    pub p999: f64,
    pub max: f64,
}

impl Summary {
    pub fn of(mut v: Vec<f64>) -> Summary {
        if v.is_empty() {
            return Summary::default();
        }
        assert!(v.iter().all(|x| !x.is_nan()), "NaN in error samples");
        v.sort_by(f64::total_cmp);
        let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        Summary {
            mean: v.iter().sum::<f64>() / v.len() as f64,
            p50: q(0.5),
            p99: q(0.99),
            p999: q(0.999),
            max: *v.last().unwrap(),
        }
    }
}

/// Result of running a codec over a color set.
#[derive(Clone, Debug)]
pub struct Eval {
    pub ok: Summary,
    pub de2000: Summary,
    pub bits_mean: f64,
    pub bits_max: u32,
}

/// Run `f` over `items` on all cores, preserving order.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> =
            items.chunks(chunk).map(|c| s.spawn(|| c.iter().map(&f).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

pub fn evaluate(codec: &dyn Codec, colors: &[Srgb]) -> Eval {
    let rows = par_map(colors, |&c| {
        let code = codec.encode(c);
        let d = codec.decode(code);
        (delta_e_ok(c, d), delta_e_2000_srgb(c, d), code.len)
    });
    let bits_mean = rows.iter().map(|r| r.2 as f64).sum::<f64>() / rows.len() as f64;
    let bits_max = rows.iter().map(|r| r.2).max().unwrap_or(0);
    Eval {
        ok: Summary::of(rows.iter().map(|r| r.0).collect()),
        de2000: Summary::of(rows.iter().map(|r| r.1).collect()),
        bits_mean,
        bits_max,
    }
}
