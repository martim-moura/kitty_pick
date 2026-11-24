# Performance notes

Measured on Darwin arm64, 2026-11-24, Kitty 0.50.0.
Release build: Rust nightly 1.102.0-nightly (20fe935d5 2026-11-23),
skim 5.9.0. These are exploratory samples, not a statistically robust benchmark.

## Kitty launch paths

The probe starts the same Python diagnostic child in each surface. The child
writes a marker and exits after 150 ms. Timing starts before launching the
Kitty client and stops when the marker is observed. Remote launches use
`--dont-take-focus`. Three samples per surface, sequential runs, current
Kitty configuration. Neither rendered pixels nor first usable input are measured.

Median child-startup latency: overlay 115 ms, OS window 149 ms, centered panel
120 ms, standalone Kitty 644 ms. Single-instance without a compatible retained
instance measured 628 ms; with a deliberately retained instance in a dedicated
group, 142 ms. Raw samples and nearest-rank p95 values are in
`launch-results.json` and `launch-warm-results.json`. With three samples,
p95 is just the maximum, not a reliable tail estimate.

The retained-instance test kept a Kitty process alive with a diagnostic child,
then ran `launch_probe.py --surface single-instance --instance-group GROUP
--run --repetitions 3`. The keeper was terminated afterward. No daemon or
persistent configuration was installed.

All three remote surfaces were also tested with the actual release
`kitty-pick launch` command, not just the probe: the real menu opened and
closed on Escape. Those tests are functional checks, not latency measurements.

## Picker and source paths

Ten warm release-menu launches on private PTYs produced the first captured
`Tools>` output in a median 54 ms, range 47–58 ms. This includes Python
supervisor startup and measures bytes received, not a rendered frame.
The first release run observed immediately after building took 884 ms; the
first run after a later rebuild took 370 ms. These observations and subsequent
warm timings are not interchangeable;
no cause is attributed to the difference.

A synthetic source with 10,000 empty files in ten directories was enumerated
five times. Median enumeration was 9.122 ms, with a median first emitted
candidate at 796 microseconds. The source-only benchmark excludes skim
matching, terminal setup, channel transport, and file creation.
Raw values are in `picker-results.json`.

Reproduce the source measurement against a chosen disposable tree:

```sh
cargo build --release --example bench_source
./target/release/examples/bench_source /path/to/test-tree
```

The standard-library Python PTY test can be imported to repeat startup
measurements. For a useful latency study, fix the candidate set, query,
terminal dimensions, and warm/cold state; measure input-to-output latency
separately. Do not equate enumeration throughput with fuzzy matcher throughput.

## Initial choice and further tests

Overlay remains the initial default: in this small sample it avoided the
standalone startup cost, without another OS window. Panels remain a supported
choice; this sample does not establish a meaningful overlay-versus-panel
advantage.

For a hotkey, use Kitty's native `launch` action. It removes the
`kitty @` client and parent `kitty-pick launch` process from the path.
That process-count reduction is structural; native-action first-frame latency
has not been measured here.

Embedding skim eliminates a `sk` process for each transition and preserves
typed candidates. It has not been demonstrated to improve matcher throughput
over the CLI. Keep default image, IPC, and CLI features disabled, and avoid
previews until they have cancellation and byte/time budgets. The current skim
default matcher is Arinae, available without optional features.

Before adding a daemon or index, measure first-frame latency, input latency
at 10k/100k/1M candidates, peak RSS, repeated directory transitions, and
cancellation on local and slow filesystems. Compare a native overlay binding
with a retained single-instance window under the same workload. Add lifecycle
and IPC complexity only if those controlled measurements justify it.
