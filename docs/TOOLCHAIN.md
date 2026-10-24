# Toolchain Setup and Environment Notes

## Verified Working Test Command

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "C:\Users\Dragonfly\Downloads\Download Manager"
cargo +stable-x86_64-pc-windows-gnu test -p sayl-core
```

## Verified Working Check Command

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "C:\Users\Dragonfly\Downloads\Download Manager"
cargo +stable-x86_64-pc-windows-gnu check -p <crate>
```

## Why This Setup Is Needed

- The MSVC toolchain (14.44.35207) and `link.exe` are installed at `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`.
- However, the Windows SDK is NOT installed, so `kernel32.lib` is missing and MSVC linking fails with `LNK1181`.
- The session is not elevated, so the Windows SDK cannot be installed by the agent.
- The `stable-x86_64-pc-windows-gnu` toolchain paired with bundled `rust-lld` links successfully with no Windows SDK requirement.

## Scope Warning

GNU-linked binaries are fine for unit and integration tests, but the final Tauri/MSVC production build will still require the Windows SDK, which needs one elevated install.
