#!/usr/bin/env python3
"""Exercise the actual skim UI on a private PTY, without opening GUI windows."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time


class Session:
    def __init__(self, binary, cwd, mode, env=None):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
        self.screen = b""
        self.started = time.perf_counter()

        def controlling_tty():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        # A supervisor keeps the controlling terminal alive for the termios
        # check. macOS revokes it when the session leader exits.
        supervisor = (
            "import subprocess, sys, termios; "
            "before = termios.tcgetattr(0); "
            "code = subprocess.call(sys.argv[1:]); "
            "after = termios.tcgetattr(0); "
            # PENDIN is a kernel-maintained pending-input flag, not a raw-mode setting.
            "before[3] &= ~getattr(termios, 'PENDIN', 0); "
            "after[3] &= ~getattr(termios, 'PENDIN', 0); "
            "print('termios diff: ' + repr([(i, a, b) for i, (a, b) in enumerate(zip(before, after)) if a != b]), file=sys.stderr) if before != after else None; "
            "sys.exit(code if before == after else 99)"
        )
        self.child = subprocess.Popen(
            [sys.executable, "-c", supervisor, str(binary), "--cwd", str(cwd), mode],
            stdin=self.slave, stdout=subprocess.PIPE, stderr=self.slave,
            env={**os.environ, "TERM": "xterm-256color", **(env or {})},
            preexec_fn=controlling_tty,
        )

    def until(self, text, timeout=8):
        end = time.monotonic() + timeout
        needle = text.encode()
        while needle not in self.screen:
            if time.monotonic() > end:
                raise AssertionError(f"UI did not show {text!r}: {self.screen[-1500:]!r}")
            if select.select([self.master], [], [], 0.05)[0]:
                self.screen += os.read(self.master, 65536)
            elif self.child.poll() is not None:
                raise AssertionError(f"picker exited {self.child.returncode}: {self.screen[-1500:]!r}")
        return (time.perf_counter() - self.started) * 1000

    def send(self, keys):
        self.screen = b""
        os.write(self.master, keys)
        time.sleep(0.06)

    def finish(self, expected=0):
        # Drain the terminal so a full PTY output buffer cannot block teardown.
        end = time.monotonic() + 8
        while self.child.poll() is None and time.monotonic() < end:
            if select.select([self.master], [], [], 0.05)[0]:
                self.screen += os.read(self.master, 65536)
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
            self.child.wait()
            raise AssertionError("picker failed to exit")
        output = self.child.stdout.read()
        assert self.child.returncode == expected, (self.child.returncode, self.screen[-1500:])
        return output

    def close(self):
        if self.child.poll() is None:
            os.killpg(self.child.pid, signal.SIGKILL)
            self.child.wait()
        self.child.stdout.close()
        os.close(self.master)
        os.close(self.slave)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", type=Path, default=Path("target/debug/picc-menu"))
    args = parser.parse_args()
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix="picc-pty-") as tmp:
        root = Path(tmp).resolve()
        child = root / "unique-child"
        child.mkdir()
        with Session(binary, root, "menu") as session:
            first_ms = session.until("Tools>")
            session.send(b"Browse")
            session.send(b"\r")
            session.until("Directories>")
            session.send(b"\x1b")
            session.until("Tools>")
            session.send(b"\x1b")
            assert session.finish() == b""
        with Session(binary, root, "choose-dir") as session:
            session.until("Directories>")
            session.send(b"unique-child")
            session.send(b"\r")
            session.until("./ (current directory)")
            session.send(b"\x0f")  # Ctrl-o, choose current directory
            assert session.finish() == os.fsencode(child) + b"\0"
        with Session(binary, root, "choose-dir") as session:
            session.until("Directories>")
            session.send(b"\x1b")
            assert session.finish(130) == b""
        with Session(binary, root, "menu") as session:
            session.until("Tools>")
            session.send(b"\x03")  # Ctrl-c must also restore terminal state.
            assert session.finish() == b""

        # Stub Neovim checks argv, not shell-escaped display text.
        bindir = root / "bin"
        bindir.mkdir()
        editor = bindir / "nvim"
        result = root / "editor-args.json"
        editor.write_text(
            "#!/usr/bin/env python3\nimport json, os, sys\n"
            "with open(os.environ['PICC_TEST_OUTPUT'], 'w') as f: json.dump(sys.argv[1:], f)\n"
        )
        editor.chmod(0o755)
        files = [root / "alpha\n'one", root / "beta;two"]
        for path in files:
            path.write_text("")
        env = {"PATH": str(bindir) + os.pathsep + os.environ["PATH"], "PICC_TEST_OUTPUT": str(result)}
        with Session(binary, root, "edit", env) as session:
            session.until("Files>")
            session.send(b"alpha")
            session.send(b"\t")
            session.send(b"\x15beta")  # Ctrl-u clears the query, retains multi-selection
            session.send(b"\t")
            session.send(b"\r")
            session.finish()
        assert json.loads(result.read_text()) == ["--", *(str(path) for path in files)]

        shell = bindir / "test-shell"
        shell_result = root / "shell-result.json"
        shell.write_text(
            "#!/usr/bin/env python3\nimport json, os, sys\n"
            "with open(os.environ['PICC_TEST_OUTPUT'], 'w') as f: "
            "json.dump({'cwd': os.getcwd(), 'args': sys.argv[1:]}, f)\n"
        )
        shell.chmod(0o755)
        with Session(binary, root, "browse", {
            "SHELL": str(shell), "PICC_TEST_OUTPUT": str(shell_result)
        }) as session:
            session.until("Directories>")
            session.send(b"unique-child")
            session.send(b"\x0f")
            session.finish()
        assert json.loads(shell_result.read_text()) == {"cwd": str(child), "args": ["-i"]}
    print(json.dumps({"pty_workflows": 6, "first_menu_output_ms": round(first_ms, 2),
                      "terminal_restored": True}))


if __name__ == "__main__":
    main()
