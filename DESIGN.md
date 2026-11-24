# Terminal menu: implementation plan

Status: planned before implementation. Project `~/picc_menu`, binary
`picc-menu`, library `picc_menu`. Picker: [skim](https://github.com/skim-rs/skim),
crate `skim`, executable `~/.cargo/bin/sk` (installed version 5.9.0).

## First implementation

1. Separate context, tool registry, picker protocol, candidate sources, and
   effects. Use stable tool IDs and typed path payloads, never parse labels
   back into commands or filenames. Make the picker replaceable for unit tests.
2. Embed skim 5.9 with default features disabled. Stream file candidates from
   an ignore-aware walker into a bounded channel. Keep one picker active at a
   time; restore the terminal before starting another picker or any effect.
3. Implement a contextual menu, multi-file Neovim editing, immediate-child
   directory browsing, opening a directory in Neovim or an interactive shell,
   and a NUL-delimited directory-selection mode for shell integration.
4. Provide explicit Kitty launch surfaces and an opt-in launch probe. Keep the
   menu and subsequent pickers in the same child process and Kitty window.
5. Test the protocol with a fake picker, path handling, traversal policy,
   cancellation/backpressure, argument construction, and actual PTY interaction.
   Run formatting, tests, Clippy, release build, and `git diff --check`.

## Boundaries

Context owns an absolute working directory and traversal options. Tools expose
an ID, label, availability predicate, and a synchronous interaction returning
an optional effect. A registry checks ID uniqueness. The menu lists available
tools without starting their sources. A picker request owns its source, prompt,
header, selection mode, and named accept bindings. Its response is cancellation
or typed entries plus the accept action. Tool cancellation returns to the menu;
menu cancellation exits. Direct tool invocation exits on cancellation.

Sources run on one worker thread per picker, send through a bounded queue,
and stop cooperatively when the picker exits. The worker must be joined even
when the picker fails. Directory enumeration uses only immediate children;
recursive file search respects ignore files, excludes hidden paths by default,
and does not traverse symlink directories. Paths remain `PathBuf` throughout.
Control characters are escaped only for display. Filesystem errors must not
silently turn a failed source into an apparently empty successful result.

Effects are data, executed only after skim releases the TTY. External programs
receive argument arrays, not shell command strings. Neovim receives `--` before
paths. A shell is started with its working directory already set. No source,
preview, or match operation spawns an external process. There is no daemon,
cache, network service, arbitrary command plugin loader, or preview process in
this version. Skim still retains received candidates for matching; a bounded
producer queue does not bound total picker memory. Cancellation cannot interrupt
an operating-system filesystem call blocked on a slow mount.

A child cannot change its parent shell's cwd. The directory browser opens a
new interactive shell in its own terminal; `choose-dir` returns a NUL-delimited
path for an explicit Fish wrapper to `cd` in the calling shell. No keystroke
injection, `eval`, or remote-control `send-text` is used.

## Kitty launch choices

`inline`: no terminal creation or Kitty remote-control request. Baseline for
measuring picker startup, and required for parent-shell directory selection.

`overlay`: reuse the current Kitty OS window and create a child PTY overlay.
Preferred initial surface inside Kitty because it avoids creating another OS
window. Pin the source window by `KITTY_WINDOW_ID` rather than focus when
available. Remote control must already be authorized; do not modify Kitty's
security configuration. Closing the child removes its overlay.

`os-window`: use `kitty @ launch --type=os-window` in the existing Kitty
process. Useful for long editor/shell sessions without hiding the origin.

`panel`: use `kitty @ launch --type=os-panel` with a centered, sized panel.
The installed Kitty 0.50.0 panel help explicitly supports macOS and Wayland
layer-shell. This support is version/platform dependent. Fail explicitly if
the launch is unsupported; do not silently substitute another surface.

`standalone`: start another Kitty process, not remote control. Works without
remote-control authorization, but initializes another terminal instance.
Use this as a fallback chosen by the user, not an automatic retry that could
create duplicate windows after a transport timeout.

The library and binary share one process across menu transitions, eliminating
per-picker `sk` process startup and path serialization. A retained window plus
a resident worker could avoid repeated process startup, but adds IPC,
concurrency, stale-context, focus, and lifecycle problems. Defer that complexity
until measurements justify it. Kitty's single-instance mode or a persistent
panel should be evaluated by controlled warm/cold tests before adding a daemon.

## Measurement, not assumptions

Record the installed versions and separately measure candidate enumeration,
first visible menu/first candidate, query latency, and launch lifetime. An
optional probe launches only a short-lived diagnostic child and compares
remote-control overlay, OS window, panel, and standalone. Measure to a child
startup marker and to teardown; neither is a first-frame measurement. Use
multiple repetitions, report median and p95, and keep the child command,
working directory, config, and machine load fixed. Compare inline binary startup
and CLI `sk` separately if investigating embedding overhead. Do not equate
window-creation latency with fuzzy-match throughput or claim that a faster
surface has been established from API shape alone.

## Subsequent extensions

Add Rust `Tool` implementations and source types without changing launch or
TTY management. Candidate next tools are Git roots/worktrees, recent paths,
and process selection. Add previews with explicit cancellation and byte/time
budgets, and history with stable typed identities. Do not adopt a persisted
filesystem index without a freshness and invalidation design.
