# Picc menu

A Rust program and library for picker-driven terminal tools, using
[skim](https://github.com/skim-rs/skim) directly. Inspired by the Neovim
`pick_menu` and [skim-run](https://github.com/skim-rs/skim-run).
No `sk` subprocess is needed at runtime.

## Build and run

Rustup selects nightly from `rust-toolchain.toml`, including rustfmt, Clippy,
rust-src, and Rust Analyzer. Dependencies use stable crates.io releases and
Cargo-compatible version ranges; `Cargo.lock` records the tested resolution.
Run `cargo update` to refresh it.

```sh
cd ~/picc_menu
cargo build --release --locked
./target/release/picc-menu
./target/release/picc-menu --cwd ~/some-project edit
./target/release/picc-menu browse
./target/release/picc-menu --hidden browse
./target/release/picc-menu tools
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
./target/release/picc-menu launch
./target/release/picc-menu --cwd ~/some-project launch --surface overlay edit
./target/release/picc-menu launch --surface os-window browse
./target/release/picc-menu launch --surface panel
./target/release/picc-menu launch --surface standalone
./target/release/picc-menu launch --surface single-instance
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
a shell invoking `picc-menu launch`. It avoids starting the remote-control
client and the parent picker launcher:

```conf
# Replace the executable path if you install elsewhere.
map kitty_mod+p launch --type=overlay --cwd=current /Users/martim.moura/picc_menu/target/release/picc-menu
```

An optional snippet is in `integration/kitty.conf`. Nothing has been added to
your active Kitty config. One process and Kitty window serve the menu and all
subsequent pickers. The selected editor or interactive shell replaces the
picker process on Unix, so it remains in that same window without an idle
wrapper process.

## Change the calling shell's directory

A child process cannot change its parent's cwd. `browse` opens a **new**
interactive shell in the selected directory; exiting it returns to the
original shell. To change the original Fish shell instead:

```fish
fish_add_path ~/picc_menu/target/release
source ~/picc_menu/integration/picc-cd.fish
picc-cd
```

The wrapper runs `picc-menu choose-dir` inline and consumes one raw
NUL-delimited path. In this mode Ctrl-o chooses a directory; no editor or
shell is started. Cancellation writes nothing and exits 130. Spaces, quotes,
and newlines do not require `eval` or shell escaping. Do not launch
`choose-dir` in a detached window expecting its output to change your shell.

## Library structure

`Context` owns the absolute cwd and hidden-path policy. `Registry` holds
Rust `Tool` implementations with stable IDs, labels, and availability checks.
A tool issues `Request` values through the `Picker` trait and returns an
optional `Effect`. `SkimPicker` is the production adapter; tests substitute
a fake picker.

A request owns a `Source`, prompt, header, multi-selection setting, and
named accept bindings. A source emits `Entry` values and stops when the
sink returns false. Entries hold a display label plus a typed `Value`;
paths are never reconstructed from their labels. Built-in sources are
`Vec<Entry>`, recursive `Files`, and immediate-child `Directories`.

The adapter sends the first candidate immediately, then batches up to 64
items through an eight-batch bounded queue. Producer cancellation and joining
are centralized. Skim still retains received candidates for matching; this is
not a bounded-total-memory index. All terminal operations stay on the calling
thread. Tool effects execute only after the picker releases the terminal.

Add a tool by implementing `tools::Tool` and calling `Registry::register`.
It can issue multiple sequential picker requests without changing the engine
or Kitty code. `examples/custom_tool.rs` is a complete example.

```sh
cargo run --example custom_tool
```

Built-in effects use argument arrays, never interpolated shell commands.
Neovim receives `--` before paths. `SKIM_DEFAULT_OPTIONS` is intentionally
not imported: environment bindings must not replace the framework's controls
or introduce command execution. Preview subprocesses, a resident daemon,
history, cached indexes, and dynamic plugins are deliberately out of scope.

A source error is returned when the active picker closes; it does not yet
interrupt the UI. Filesystem syscalls blocked on a slow mount cannot be
interrupted by cooperative cancellation. These are current limitations, not
promises of instantaneous cancellation.

## Development

Clippy uses its standard lint set. Rustfmt uses its defaults. The native
`rust-analyzer.toml` selects Clippy diagnostics on save; the minimal
`.vscode/settings.json` supplies the equivalent editor setting. No global
editor config is required or modified.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --no-deps -- -D warnings
cargo test --all-targets --locked
python3 scripts/pty_smoke.py target/debug/picc-menu
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

See `DESIGN.md` for the pre-implementation plan, `docs/skim-run.md` for the
reference review, and `docs/performance.md` for measured launch results,
methodology, and remaining performance work.
