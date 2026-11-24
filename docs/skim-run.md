# Reference: skim-run

Inspected https://github.com/skim-rs/skim-run at
`264345a6d7be886dbd0b8bd0dc7c64b0b7e3bc3e`, cloned to `~/skim-run`.
The corresponding skim upstream is https://github.com/skim-rs/skim.
No implementation code was copied; skim-run is not a dependency.

## Useful patterns

`src/lib.rs` defines `SkimRun` with `init`, `get`, `set_options`, and
`run` hooks. Each module owns a tool instead of coupling all tool logic to
the CLI. `src/main.rs` configures one skim invocation, gets its result, and
then dispatches it. Named accept actions support switching modes without
another executable invocation. Candidate types implement `SkimItem`, keeping
matching/display text distinct from output identity.

Kitty pick applies those ideas through `Tool`, `Source`, `Picker`, and
`Effect`. Tool-specific Rust code owns the interaction; shared code owns
candidate transport, terminal entry/exit, and launching. Named accept actions
implement browse/open/edit and parent navigation.

## Deliberate differences

The reference uses skim 0.20.2. Kitty pick pins skim 5.9.0, matching the installed
`~/.cargo/bin/sk`; that API takes owned options and receives batches of items.

The reference's `get` returns a complete vector before skim starts. Kitty pick
streams from a worker with bounded batches, sends the first candidate
immediately, checks cancellation between candidates, and joins the worker.
This avoids waiting for a full recursive walk before showing the picker.

The reference's mode transitions substitute strings into an accept action,
split on spaces, and update CLI arguments. Kitty pick passes typed path
payloads and effects instead. Spaces, quotes, control characters, and
non-UTF-8 path bytes do not need an escaping/parsing round trip.

The reference lets tools mutate arbitrary skim options, parse
`SKIM_DEFAULT_OPTIONS`, and execute shell bindings. Kitty pick exposes a
smaller request protocol and intentionally does not import environment
options that could replace its bindings or start commands. All built-in
effects use argument vectors after the picker closes.

The reference's main dispatcher discards errors from `mode.run`. Kitty pick
propagates tool, source, picker, and launch errors. Source errors are returned
when the active picker closes; they do not yet interrupt its UI.

Linux desktop modes, application icon previewing, calculator networking,
systemd, and Hyprland integration are not brought into this macOS/Kitty-first
project. Optional tool dependencies and compile-time modules remain a useful
direction if the built-in set grows.
