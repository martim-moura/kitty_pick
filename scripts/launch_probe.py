#!/usr/bin/env python3
"""Opt-in Kitty launch probe. Opens short-lived windows; no picker or first-frame measurement."""
import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


SURFACES = ("overlay", "os-window", "panel", "standalone", "single-instance")


def launch_command(surface, marker, child, instance_group=None):
    # The same diagnostic child is used for every surface. It writes one marker
    # immediately, stays alive 150 ms, and then removes its window by exiting.
    diagnostic = [sys.executable, str(child), "--child", str(marker)]
    if surface in ("standalone", "single-instance"):
        command = ["kitty"]
        if surface == "single-instance":
            command += ["--single-instance", "--wait-for-single-instance-window-close"]
            if instance_group:
                command += ["--instance-group", instance_group]
        return command + ["--directory", str(marker.parent), "--title", "Picc launch probe"] + diagnostic
    command = ["kitty", "@", "launch", "--type", "os-panel" if surface == "panel" else surface,
               "--dont-take-focus", "--wait-for-child-to-exit", "--response-timeout", "15",
               "--title", "Picc launch probe", "--cwd", str(marker.parent)]
    if window := os.environ.get("KITTY_WINDOW_ID"):
        command += ["--source-window", "id:" + window, "--match", "window_id:" + window]
    if surface == "panel":
        command += ["--os-panel", "edge=center-sized", "--os-panel", "lines=28",
                    "--os-panel", "columns=100", "--os-panel", "layer=overlay"]
    return command + diagnostic


def percentile(values, p):
    return sorted(values)[max(0, math.ceil(len(values) * p) - 1)]


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--child":
        Path(sys.argv[2]).write_text("started")
        time.sleep(0.15)
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--surface", choices=SURFACES, action="append", dest="surfaces")
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--instance-group", help="existing Kitty single-instance group, for controlled warm tests")
    parser.add_argument("--run", action="store_true", help="actually create GUI windows; otherwise print commands")
    args = parser.parse_args()
    if args.repetitions < 1:
        parser.error("--repetitions must be positive")
    surfaces = args.surfaces or list(SURFACES)
    with tempfile.TemporaryDirectory(prefix="picc-launch-") as tmp:
        # Keep child entry point accessible to independent Kitty processes.
        child = Path(__file__).resolve()
        results = []
        for surface in surfaces:
            samples = []
            for n in range(args.repetitions if args.run else 1):
                marker = Path(tmp) / f"{surface}-{n}"
                command = launch_command(surface, marker, child, args.instance_group)
                if not args.run:
                    print(json.dumps({"surface": surface, "argv": command}))
                    continue
                start = time.perf_counter()
                process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                ready_ms = None
                deadline = time.monotonic() + 20
                try:
                    while time.monotonic() < deadline:
                        if marker.exists():
                            ready_ms = (time.perf_counter() - start) * 1000
                            break
                        if process.poll() is not None:
                            break
                        time.sleep(0.001)
                    stdout, stderr = process.communicate(timeout=max(1, deadline - time.monotonic()))
                    lifetime_ms = (time.perf_counter() - start) * 1000
                    remote_ok = surface in ("standalone", "single-instance") or stdout.strip() == b"0"
                    if ready_ms is None or process.returncode or not remote_ok:
                        raise RuntimeError(f"exit={process.returncode}, stdout={stdout!r}, stderr={stderr!r}")
                    samples.append({"child_started_ms": ready_ms, "lifetime_ms": lifetime_ms})
                except (RuntimeError, subprocess.TimeoutExpired) as error:
                    process.kill()
                    process.communicate()
                    results.append({"surface": surface, "error": str(error)})
                    break
            if samples:
                starts = [s["child_started_ms"] for s in samples]
                results.append({"surface": surface, "samples": samples,
                                "median_child_started_ms": statistics.median(starts),
                                "p95_child_started_ms": percentile(starts, 0.95)})
        if args.run:
            print(json.dumps({"kitty": subprocess.check_output(["kitty", "--version"], text=True).strip(),
                              "measurement": "child startup marker, NOT first visible frame",
                              "results": results}, indent=2))


if __name__ == "__main__":
    main()
