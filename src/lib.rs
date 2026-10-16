//! Research codecs that encode a single sRGB color into a compact bit code.
//!
//! - [`codecs::lattice`]: a BCC lattice in weighted Oklab that numbers only the in-gamut points.
//!   Best fixed-rate codec in the study.
//! - [`codecs::kd`]: a deterministic binary partition of the sRGB cube whose cuts are placed by
//!   perceptual geometry. Progressive and optionally variable-length with an error target.
//! - [`codecs::baseline`]: uniform RGB / Oklab quantization and the Archimedean spiral.
//!
//! See `README.md` for the study and results.

// Color math indexes parallel per-channel arrays; index loops read better than zips here.
#![allow(clippy::needless_range_loop)]

pub mod code;
pub mod codecs;
pub mod color;
pub mod data;
pub mod flat;
pub mod metric;
pub mod stats;
