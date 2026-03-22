# Contributing

## Dev Setup

### Prerequisites

- **Rust stable** -- install via [rustup](https://rustup.rs/)
- **Node.js 22+** and **bun** -- for the Tauri frontend
- **Tauri CLI v2**: `cargo install tauri-cli`

### Recommended tools

- **devbox** -- reproducible environment (`devbox shell`)
- **direnv** -- auto-loads `.envrc` on `cd` into the repo
- **cargo-nextest** -- faster parallel test runner (`cargo install cargo-nextest`)
- **cargo-watch** -- re-run on save (`cargo install cargo-watch`)

### Clone and build

```bash
git clone https://github.com/d-cryptic/claude-sentinel
cd claude-sentinel

# Auto-activate with direnv, or manually:
devbox shell

# Build all crates
cargo build

# Install the CLI locally
cargo install --path crates/cst-cli

# Verify
cst --version
```

### Running the Tauri desktop app

```bash
cd apps/desktop
bun install
cargo tauri dev     # development mode with hot reload
cargo tauri build   # production build
```

## Repo Structure

```
crates/
  cst-core/       Shared library -- all domain logic, no I/O side effects in tests
  cst-cli/        CLI binary (cst) -- thin layer over cst-core + TUI + integrations
apps/
  desktop/        Tauri v2 app (React + TypeScript frontend, neubrutalism UI)
    src/           React components, stores, styles
    src-tauri/     Tauri backend (Rust, wraps cst-core)
docs/             Documentation
.github/          CI workflows
```

**Key rule**: All shared logic lives in `cst-core`. Never duplicate business logic in `cst-cli` or the Tauri backend. Both consumers call into `cst-core` for profile/session/auth/daemon operations.

## Make Commands

```bash
make test           # cargo nextest run (all crates)
make test-watch     # TDD watch mode (re-runs on file change)
make build          # cargo build
make install        # cargo install --path crates/cst-cli
make lint           # clippy --all -D warnings
make fmt            # cargo fmt --all
make check          # fmt + lint + test (run before every commit)
make dev-app        # Tauri dev mode
make changelog      # generate CHANGELOG.md via git-cliff
```

## TDD Workflow

Write tests **before** implementation. Follow the red-green-refactor cycle:

1. **RED** -- write a test that describes the desired behavior. It should fail.
2. **GREEN** -- write the minimal code to make the test pass.
3. **REFACTOR** -- clean up without breaking tests.

```bash
# Watch mode (re-runs tests on every save)
make test-watch
# or directly:
cargo watch -x "nextest run"

# Run all tests
cargo nextest run

# Run tests for a specific crate
cargo nextest run -p cst-core
cargo nextest run -p cst-cli

# Run a specific test by name
cargo nextest run test_profile_crud
```

### Test organization

- **Unit tests**: `#[cfg(test)]` modules at the bottom of each source file
- **Integration tests**: `crates/*/tests/` directories

## Code Standards

1. **No `unwrap()` in production paths** -- use `?` with `anyhow::Result`
2. **`thiserror`** for library error types in `cst-core`, **`anyhow`** for binary error propagation in `cst-cli`
3. **All public functions** must have rustdoc comments with at least one `/// # Examples` block
4. **`clippy::all`** must pass with zero warnings in CI
5. **`cargo fmt --all`** must produce no diff

### Pre-commit check

Run this before every commit:

```bash
make check
```

This runs `cargo fmt --all -- --check`, then `clippy`, then the full test suite.

## Commit Convention

Format: `<type>: <description>`

```
feat: add quota warning notifications
fix: prevent panic when profiles dir is missing
docs: update auto-switch configuration guide
chore: update dependencies
test: add scheduler edge case coverage
refactor: extract rate-limit detection into detector module
perf: cache profile list to avoid repeated disk reads
ci: add aarch64-linux-gnu build target
```

Rules:
- Commit after each logical unit of work
- Never commit broken code (all tests must pass)
- Never batch unrelated changes in one commit
- Description is lowercase, imperative mood ("add" not "added" or "adds")

## Pull Requests

1. Create a feature branch from `main` (or `develop` if it exists)
2. Link the related issue in the PR description
3. Keep PRs focused on one concern
4. Self-review the diff before requesting review
5. All CI checks must pass before merge

### CI Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs on every push to `main`/`develop` and on PRs to `main`:

| Job | What it does |
|-----|--------------|
| `test` | Format check, clippy, unit tests (cst-core + cst-cli), release build, smoke test (`cst --version`) |
| `build-matrix` | Cross-compile for x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin |

Tests run on both `ubuntu-latest` and `macos-latest`.

## Adding a New cst-core Module

1. Create `crates/cst-core/src/mymodule.rs`
2. Add `pub mod mymodule;` to `crates/cst-core/src/lib.rs`
3. Write tests in `#[cfg(test)]` at the bottom of the file
4. Export any public types from `lib.rs` if needed by external consumers

## Adding a New CLI Command

1. Create `crates/cst-cli/src/commands/mycommand.rs`
2. Add `pub mod mycommand;` to `crates/cst-cli/src/commands/mod.rs`
3. Add the variant to the `Commands` enum in `main.rs` with a doc comment (used by `--help`)
4. Add the match arm in the `main()` function
5. If the command needs cst-core logic, add it there first and call from the CLI layer

## Adding a New Tauri Command

1. Add the core logic to `cst-core` (if not already there)
2. Create the Tauri command wrapper in `apps/desktop/src-tauri/src/commands/`
3. Register it in the `invoke_handler![]` macro in `apps/desktop/src-tauri/src/lib.rs`
4. Call it from the React frontend via `import { invoke } from "@tauri-apps/api/core"`

## Architecture References

- [ARCHITECTURE.md](ARCHITECTURE.md) -- crate structure, data flow, daemon design
- [AUTH.md](AUTH.md) -- authentication types and credential storage
- [AUTO-SWITCH.md](AUTO-SWITCH.md) -- daemon configuration and rate-limit detection
- [DESIGN.md](DESIGN.md) -- neubrutalism UI design system
- [USAGE.md](USAGE.md) -- full CLI command reference
