# Finance Desktop

Single-user, fully offline desktop finance application built with Tauri 2, Svelte 5, Rust, and SQLite.

## Architecture & Documentation

- [Scaffold and Command Boundary Architecture](docs/architecture/scaffold-decision.md)
- [Collaboration Protocol & Agent Guide](AGENTS.md)

## Prerequisites

- Node.js >= 22
- Rust >= 1.90 (2024 edition)

## Developer Workflow

```bash
# Install dependencies
npm install

# Run frontend checks and tests
npm run check
npm test

# Build frontend production bundle
npm run build

# Run Rust unit/integration tests and lints
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

# Compile desktop application binary
cargo build --manifest-path src-tauri/Cargo.toml
```
