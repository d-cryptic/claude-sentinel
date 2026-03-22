# Windows Installer (.msi)

Claude Sentinel ships a `.zip` binary for Windows (from GitHub Releases). A proper `.msi` installer can be built manually using [cargo-wix](https://github.com/volks73/cargo-wix).

## Prerequisites

1. [WiX Toolset v3](https://wixtoolset.org/docs/wix3/) — adds `candle.exe` and `light.exe` to PATH
2. `cargo install cargo-wix`

## Build

```bash
# From the repo root
cargo build -p cst-cli --release --target x86_64-pc-windows-msvc
cargo wix -p cst-cli --no-build
```

This produces `target/wix/cst-*.msi`.

## Install

Double-click the `.msi` or run:
```powershell
msiexec /i cst-0.1.0-x86_64.msi /quiet
```

The installer adds `cst.exe` to `%ProgramFiles%\claude-sentinel\` and appends
it to the system `PATH`.

## Uninstall

Via Control Panel > Programs and Features > Claude Sentinel > Uninstall, or:
```powershell
msiexec /x cst-0.1.0-x86_64.msi /quiet
```

## WiX Template

The WiX source is at `wix/main.wxs`. It references:

- `wix/License.rtf` — RTF-formatted MIT license (included)
- `wix/icon.ico` — application icon (optional, commented out by default)

To enable the icon, place a valid `.ico` file at `wix/icon.ico` and uncomment
the `Icon` and `ARPPRODUCTICON` lines in `wix/main.wxs`.
