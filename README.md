# fastrustcompiler

A rustc 1.100.0 build tuned for compile speed on Apple Silicon (M1), with benchmarks against the official 1.100.0 release.

Work in progress. Each optimization lands through a PR along with its benchmark numbers.

## Layout
- `config/` — bootstrap `config.toml` for the tuned build
- `patches/` — source patches applied on top of rust 1.100.0
- `bench/` — benchmark harness (ripgrep: check / debug / release)
- `results/` — benchmark results
