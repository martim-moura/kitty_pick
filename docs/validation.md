# Validation environment

Rust dependencies were checked against crates.io's latest stable versions:
skim 5.9.0, clap 4.6.9, ignore 0.4.35, anyhow 1.0.105, tempfile 3.27.0.
The manifest uses compatible version ranges rather than exact `=` pins.
Some transitive packages remain below their latest release because of upstream
version constraints; no dependency overrides were added.

Final checks passed: default rustfmt, Clippy with warnings denied, 17 Rust
tests, six real-PTY workflows against both debug and release builds, Fish
syntax validation, and `git diff --check`. Actual release menus were launched
and cancelled in Kitty overlays, OS windows, and centered panels.

The project's `rust-toolchain.toml` selects nightly. Resolving its requested
components updated the already installed nightly channel to
`rustc 1.102.0-nightly (20fe935d5 2026-11-23)`.

The machine's inherited Cargo configuration redirects crates.io to a private
Artifactory registry without a usable token in this session. No credentials
or global configuration were changed. Verification used a temporary
`CARGO_HOME` without that configuration and invoked Cargo from `/tmp`
with `--manifest-path /Users/martim.moura/kitty_pick/Cargo.toml`. Running from
`/tmp` matters: Cargo also discovers configuration in parent directories of
the current working directory, independently of `CARGO_HOME`.

If the inherited registry remains unauthenticated, normal Cargo invocations
inside the project can still fail before compiling. Either authenticate that
registry through the usual local workflow or use an isolated Cargo home:

```sh
build_home=$(mktemp -d)
cd /tmp
CARGO_HOME="$build_home" cargo +nightly build --release --locked \
  --manifest-path "$HOME/kitty_pick/Cargo.toml"
```

The latter downloads dependencies from crates.io into the temporary home.
The already built executable at `~/kitty_pick/target/release/kitty-pick`
does not need Cargo, Rust, Python, or `sk` at runtime.
