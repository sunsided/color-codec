//! Bit codes and the common codec interface.

use crate::color::Srgb;

/// A variable-length bit string of up to 64 bits.
///
/// Bits are stored MSB-first in the low `len` bits of `bits`: the first emitted bit is
/// bit `len - 1`, the last emitted bit is bit `0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Code {
    pub bits: u64,
    pub len: u32,
}

impl Code {
    pub const MAX_LEN: u32 = 64;

    #[inline]
    pub fn push(&mut self, bit: bool) {
        debug_assert!(self.len < Self::MAX_LEN);
        self.bits = (self.bits << 1) | bit as u64;
        self.len += 1;
    }

    /// Bit `i` in emission order (`0` is the first bit emitted).
    #[inline]
    pub fn bit(&self, i: u32) -> bool {
        debug_assert!(i < self.len);
        (self.bits >> (self.len - 1 - i)) & 1 == 1
    }

    /// The first `n` emitted bits.
    #[inline]
    pub fn prefix(&self, n: u32) -> Code {
        let n = n.min(self.len);
        Code { bits: if n == 0 { 0 } else { self.bits >> (self.len - n) }, len: n }
    }

    /// Append the low `n` bits of `v`, most significant first.
    #[inline]
    pub fn push_bits(&mut self, v: u64, n: u32) {
        for i in (0..n).rev() {
            self.push((v >> i) & 1 == 1);
        }
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in 0..self.len {
            f.write_str(if self.bit(i) { "1" } else { "0" })?;
        }
        Ok(())
    }
}

/// A single-color codec: maps an sRGB color to a bit code and back.
pub trait Codec: Sync {
    fn name(&self) -> String;
    fn encode(&self, c: Srgb) -> Code;
    fn decode(&self, code: Code) -> Srgb;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_bit_prefix_agree() {
        let mut c = Code::default();
        for b in [true, false, true, true, false] {
            c.push(b);
        }
        assert_eq!(c.to_string(), "10110");
        assert_eq!(c.prefix(3).to_string(), "101");
        assert_eq!(c.prefix(0).len, 0);
        let mut d = Code::default();
        d.push_bits(0b10110, 5);
        assert_eq!(c, d);
    }
}
