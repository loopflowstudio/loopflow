# Rust-First Release: Binary Distribution

Ship `lf` and `lfd` as Rust binaries via PyPI. Follow the ruff/uv model.

## Status: Ready for CI

Local build and install verified:
```bash
uv run maturin build --release
uv tool install target/wheels/loopflow-*.whl
lf --help  # works
lfd        # starts daemon
```

## Configuration

**pyproject.toml:**
```toml
[build-system]
requires = ["maturin>=1.4,<2.0"]
build-backend = "maturin"

[tool.maturin]
bindings = "bin"
manifest-path = "rust/lf/Cargo.toml"
strip = true
```

**Crate structure:**
```
rust/lf/
├── Cargo.toml          # [[bin]] lf, [[bin]] lfd
├── build.rs            # tonic/protobuf generation
└── src/
    ├── main.rs         # lf entrypoint
    ├── lfd_main.rs     # lfd entrypoint
    ├── lfd/            # lfd modules (merged from old lfd crate)
    └── commands/       # lf commands
```

## Wheel Contents

```
loopflow-0.8.0-py3-none-macosx_11_0_arm64.whl
├── loopflow-0.8.0.data/
│   └── scripts/
│       ├── lf    (5.6MB stripped)
│       └── lfd   (11.3MB stripped)
└── loopflow-0.8.0.dist-info/
```

## Next: CI/CD

Update `.github/workflows/release.yml` to:

```yaml
name: Release
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
          - os: macos-13
            target: x86_64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: PyO3/maturin-action@v1
        with:
          target: ${{ matrix.target }}
          args: --release --out dist
          manylinux: auto
      - uses: actions/upload-artifact@v4
        with:
          name: wheels-${{ matrix.target }}
          path: dist/*.whl

  publish:
    needs: build
    runs-on: ubuntu-latest
    environment: pypi
    steps:
      - uses: actions/download-artifact@v4
        with:
          pattern: wheels-*
          merge-multiple: true
          path: dist
      - uses: pypa/gh-action-pypi-publish@release/v1
        with:
          packages-dir: dist/
```

## Completed

- [x] Remove PyO3 bindings from loopflow-engine
- [x] Merge lfd crate into lf crate
- [x] Configure maturin for binary distribution
- [x] Local build and install test
- [ ] Update release workflow
- [ ] Test CI build
- [ ] Publish to PyPI
