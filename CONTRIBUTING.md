# Contributing to OxyConvert

Thank you for your interest in contributing to OxyConvert!

## Development Setup

1. **Rust Toolchain**: OxyConvert requires Rust 1.88+ (pinned to 1.99.0 in `rust-toolchain.toml`).
2. **Build and Test**:
   ```bash
   cargo check
   cargo test
   cargo clippy --all-targets --all-features
   ```

## Code Guidelines

- **Safety**: `unsafe_code = "forbid"` is enabled project-wide. All code must be 100% safe Rust.
- **Styling & Design Language**: OxyConvert follows the exact design language, palette presets, and corner radii of OxyBrush.
- **Testing**: Add unit tests for every new converter command parser or transpiler rule.
- **Pull Requests**: Explain the purpose of changes and ensure tests and clippy pass with zero warnings.

## Licensing

Contributions are licensed under either:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
