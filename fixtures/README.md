# Test Fixtures

Deterministic fixtures for protocol, chunking, range validation, and resume verification.

## sample.bin
- Size: 262144 bytes (256 KiB)
- Format: 16-byte deterministic blocks (8-byte big-endian block index + 8-byte hash prefix).
- SHA-256: `fdcd6d5b04c631e748b0fa7499dc582969146f099f680bcff215424c133f27b8`
- Generator: `generate_sample.py`
