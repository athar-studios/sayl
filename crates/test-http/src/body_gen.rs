//! Deterministic body generator for SAYL download manager tests.
//!
//! Generates repeatable byte sequences (0x00..=0xFF repeated) and provides
//! pattern verification, range slicing, and reference fixture comparison.

use bytes::Bytes;

/// Reference fixture size in bytes (256 KiB), matching `fixtures/sample.bin`.
pub const REFERENCE_SAMPLE_SIZE: usize = 262_144;

/// Generates a deterministic byte sequence where each byte `b[i] = (i % 256) as u8`.
pub fn generate_bytes(size: usize) -> Vec<u8> {
    generate_bytes_offset(0, size)
}

/// Generates a deterministic byte sequence of `size` bytes starting from the given `offset`.
/// Each byte `b[i] = ((offset + i) % 256) as u8`.
pub fn generate_bytes_offset(offset: usize, size: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(size);
    for i in 0..size {
        buf.push(((offset + i) % 256) as u8);
    }
    buf
}

/// Verifies whether `data` matches the expected deterministic pattern `(offset + i) % 256`.
pub fn verify_pattern(data: &[u8], offset: usize) -> bool {
    for (i, &byte) in data.iter().enumerate() {
        let expected = ((offset + i) % 256) as u8;
        if byte != expected {
            return false;
        }
    }
    true
}

/// Deterministic body generator managing size and ranged slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyGenerator {
    total_size: usize,
}

impl BodyGenerator {
    /// Creates a new generator with the specified total byte size.
    pub fn new(total_size: usize) -> Self {
        Self { total_size }
    }

    /// Total size in bytes.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Returns the full deterministic body as a `Bytes` buffer.
    pub fn full_bytes(&self) -> Bytes {
        Bytes::from(generate_bytes(self.total_size))
    }

    /// Slices the deterministic body across `[start, end)`.
    ///
    /// If `start >= total_size`, returns empty bytes.
    /// Clamps `end` to `total_size`.
    pub fn slice(&self, start: usize, end: usize) -> Bytes {
        if start >= self.total_size || start >= end {
            return Bytes::new();
        }
        let clamped_end = end.min(self.total_size);
        let len = clamped_end - start;
        Bytes::from(generate_bytes_offset(start, len))
    }

    /// Slices using standard HTTP range inclusive bounds `[first_byte, last_byte]`.
    pub fn slice_http_range(&self, first_byte: usize, last_byte: usize) -> Bytes {
        if first_byte >= self.total_size || first_byte > last_byte {
            return Bytes::new();
        }
        let end = (last_byte + 1).min(self.total_size);
        self.slice(first_byte, end)
    }

    /// Verifies that `data` matches the pattern at `offset`.
    pub fn verify(&self, data: &[u8], offset: usize) -> bool {
        verify_pattern(data, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bytes_empty() {
        let b = generate_bytes(0);
        assert!(b.is_empty());
    }

    #[test]
    fn test_generate_bytes_sequence() {
        let b = generate_bytes(300);
        assert_eq!(b.len(), 300);
        for (i, byte) in b.iter().take(256).enumerate() {
            assert_eq!(*byte, i as u8);
        }
        assert_eq!(b[256], 0);
        assert_eq!(b[299], 43);
    }

    #[test]
    fn test_generate_bytes_offset() {
        let b = generate_bytes_offset(250, 10);
        assert_eq!(b.len(), 10);
        assert_eq!(b[0], 250);
        assert_eq!(b[5], 255);
        assert_eq!(b[6], 0);
        assert_eq!(b[9], 3);
    }

    #[test]
    fn test_verify_pattern() {
        let valid = generate_bytes_offset(100, 50);
        assert!(verify_pattern(&valid, 100));
        assert!(!verify_pattern(&valid, 101));

        let mut invalid = valid.clone();
        invalid[25] = 0xAA;
        assert!(!verify_pattern(&invalid, 100));
    }

    #[test]
    fn test_body_generator_slice() {
        let gen = BodyGenerator::new(1000);
        assert_eq!(gen.total_size(), 1000);

        let slice = gen.slice(10, 20);
        assert_eq!(slice.len(), 10);
        assert_eq!(slice[0], 10);
        assert_eq!(slice[9], 19);

        let out_of_bounds = gen.slice(1000, 1100);
        assert!(out_of_bounds.is_empty());

        let clamped = gen.slice(990, 1050);
        assert_eq!(clamped.len(), 10);
    }

    #[test]
    fn test_slice_http_range() {
        let gen = BodyGenerator::new(500);
        let range = gen.slice_http_range(0, 99);
        assert_eq!(range.len(), 100);
        assert_eq!(range[0], 0);
        assert_eq!(range[99], 99);

        let clamped = gen.slice_http_range(450, 600);
        assert_eq!(clamped.len(), 50);
    }
}
