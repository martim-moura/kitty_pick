# Kitty pick

A launcher, simple UI engine, OS-level picker (like Telescope.nvim for the OS) using
[skim](https://github.com/skim-rs/skim) directly. Inspired by Neovim
pickers and [skim-run](https://github.com/skim-rs/skim-run), dmenu.

## Build and run

Rustup selects nightly from `rust-toolchain.toml`, including rustfmt, Clippy,
rust-src, and Rust Analyzer. Dependencies use stable crates.io releases and
Cargo-compatible version ranges; `Cargo.lock` records the tested resolution.
Run `cargo update` to refresh it.

```sh
cd ~/kitty_pick
cargo build --release --locked
./target/release/kitty-pick
./target/release/kitty-pick --cwd ~/some-project edit
./target/release/kitty-pick browse
./target/release/kitty-pick --hidden browse
./target/release/kitty-pick tools
```

The inherited private Cargo registry required credentials in this session;
`docs/validation.md` records the isolated crates.io build workaround. No global
Cargo configuration was changed.

`edit` requires `nvim` on PATH. The menu hides it when unavailable.
Choose files with Enter, or use Tab to select multiple files before Enter.
File discovery respects ignore files and excludes hidden paths by default.

The directory browser lists immediate children, not every descendant. Enter
descends into the highlighted directory, Alt-h visits its parent, Ctrl-o opens
an interactive shell there, and Ctrl-e opens it in Neovim. Select `./` to use
the current directory. Hidden paths are opt-in; directory navigation does not
apply ignore files. File walks do not follow directory symlinks; the browser
can enter one explicitly.

Esc cancels a picker. From a tool launched through the menu, cancellation
returns to the menu. From a directly invoked tool, it exits. An empty file
selection does not launch Neovim.

## Kitty surfaces

```sh
./target/release/kitty-pick launch
./target/release/kitty-pick --cwd ~/some-project launch --surface overlay edit
./target/release/kitty-pick launch --surface os-window browse
./target/release/kitty-pick launch --surface panel
./target/release/kitty-pick launch --surface standalone
./target/release/kitty-pick launch --surface single-instance
```

Overlay is the default. Overlay, OS-window, and panel launches use
`kitty @ launch` and require Kitty remote-control authorization. The origin
is pinned by `KITTY_WINDOW_ID` when available. They wait for launch
acknowledgment, not for the selected tool to finish. Launch failures are
reported without silently trying another surface.

Panel launches use Kitty's centered OS panel, tested on macOS with Kitty
0.50.0. Other Kitty versions and platforms may reject it. Standalone starts
another Kitty process without remote control; single-instance requests an
existing compatible Kitty instance, or starts one if none exists. These two
launch modes are asynchronous; errors after process creation belong to Kitty.

For repeated use inside Kitty, prefer its **native launch action** instead of
a shell invoking `kitty-pick launch`. It avoids starting the remote-control
client and the parent picker launcher:

```conf
# Replace the executable path if you install elsewhere.
map kitty_mod+p launch --type=overlay --cwd=current /Users/martim.moura/kitty_pick/target/release/kitty-pick
```




## Development

Add a tool by implementing `tools::Tool` and calling `Registry::register`.
It can issue multiple sequential picker requests without changing the engine
or Kitty code. `examples/custom_tool.rs` is a complete example.

```sh
cargo run --example custom_tool
```

Clippy uses its standard lint set. Rustfmt uses its defaults. The native
`rust-analyzer.toml` selects Clippy diagnostics on save; the minimal
`.vscode/settings.json` supplies the equivalent editor setting. No global
editor config is required or modified.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --no-deps -- -D warnings
cargo test --all-targets --locked
python3 scripts/pty_smoke.py target/debug/kitty-pick
git diff --check
```

`scripts/check.sh` runs these checks and builds the debug binary for the PTY
test. Python 3 is needed only for development scripts. The PTY suite validates
actual skim interaction and terminal-mode restoration without GUI windows.

The launch probe prints command arrays by default. It creates short-lived
Kitty windows **only** with `--run`:

```sh
python3 scripts/launch_probe.py
python3 scripts/launch_probe.py --run --repetitions 10
```
