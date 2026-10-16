//! Worst-case and p99 ΔE_OK of cubic vs BCC gamut lattices (w = 1) at equal point budgets.
use color_codec::codecs::lattice::{GamutLattice, LatticeKind};
use color_codec::data::Dataset;
use color_codec::stats::evaluate;

fn main() {
    let colors = Dataset::UniformOklab.sample(100_000, 99);
    for bits in [20, 24] {
        for kind in [LatticeKind::Cubic, LatticeKind::Bcc] {
            let lat = GamutLattice::new(kind, 1.0, bits);
            let e = evaluate(&lat, &colors);
            println!("{bits}b {kind:?}: ΔE_OK p99 {:.5} max {:.5}", e.ok.p99, e.ok.max);
        }
    }
}
