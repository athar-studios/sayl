"""Report the requested execution level embedded in a Windows installer binary.

The NSIS installer produced by the Tauri bundler embeds an application manifest;
the elevation it asks for is one of the execution-level strings below. Searching
the raw bytes is enough to answer "does this installer request admin rights?"
without running it.

Usage: python tools/check-installer-elevation.py releases/SAYL-Setup-x64-0.1.0.exe
"""

import sys
from pathlib import Path

TOKENS = [
    b"asInvoker",
    b"requireAdministrator",
    b"highestAvailable",
    b"requestedExecutionLevel",
]


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__.strip())
        return 2
    path = Path(argv[1])
    if not path.is_file():
        print(f"missing file: {path}")
        return 2
    data = path.read_bytes()
    print(f"file: {path} ({len(data)} bytes)")
    found = {}
    for token in TOKENS:
        count = data.count(token)
        found[token.decode()] = count
        print(f"  {token.decode():26} {count}")
    if found["requireAdministrator"]:
        print("verdict: the installer manifest requests administrator elevation")
    elif found["asInvoker"]:
        print("verdict: the installer manifest requests asInvoker (no UAC prompt)")
    elif found["highestAvailable"]:
        print("verdict: the installer manifest requests highestAvailable")
    else:
        print("verdict: no manifest execution level found in the raw bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
