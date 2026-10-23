#!/usr/bin/env python3
"""Generate deterministic test fixture for HTTP chunk and range download testing."""
import hashlib
from pathlib import Path

FIXTURE_DIR = Path(__file__).resolve().parent
SAMPLE_BIN = FIXTURE_DIR / "sample.bin"
SIZE_BYTES = 262144  # 256 KiB


def generate():
    # Deterministic generation: 16-byte blocks containing block index (8 bytes big-endian)
    # followed by the first 8 bytes of sha256 of that index.
    # This allows verifying any byte offset or subrange independently.
    data = bytearray(SIZE_BYTES)
    for block_idx in range(SIZE_BYTES // 16):
        idx_bytes = block_idx.to_bytes(8, byteorder="big")
        h = hashlib.sha256(idx_bytes).digest()[:8]
        offset = block_idx * 16
        data[offset:offset + 8] = idx_bytes
        data[offset + 8:offset + 16] = h

    SAMPLE_BIN.write_bytes(bytes(data))
    sha256_hex = hashlib.sha256(data).hexdigest()

    checksum_file = FIXTURE_DIR / "sample.bin.sha256"
    checksum_file.write_text(f"{sha256_hex}  sample.bin\n", encoding="utf-8")

    readme_file = FIXTURE_DIR / "README.md"
    readme_content = f"""# Test Fixtures

Deterministic fixtures for protocol, chunking, range validation, and resume verification.

## sample.bin
- Size: {SIZE_BYTES} bytes (256 KiB)
- Format: 16-byte deterministic blocks (8-byte big-endian block index + 8-byte hash prefix).
- SHA-256: `{sha256_hex}`
- Generator: `generate_sample.py`
"""
    readme_file.write_text(readme_content, encoding="utf-8")

    print(f"Generated {SAMPLE_BIN}: {SIZE_BYTES} bytes, SHA-256={sha256_hex}")


if __name__ == "__main__":
    generate()
