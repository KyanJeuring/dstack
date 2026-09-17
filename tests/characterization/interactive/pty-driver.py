#!/usr/bin/env python3
"""Run one command in a Unix PTY with scripted input and a hard timeout."""

from __future__ import annotations

import argparse
import errno
import os
import pty
import select
import signal
import sys
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--transcript", required=True)
    parser.add_argument("--status-file", required=True)
    parser.add_argument("--input-file")
    parser.add_argument("--wait-for")
    parser.add_argument("--send-eof", action="store_true")
    parser.add_argument("--timeout", type=float, default=5.0)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    return args


def child_exit_code(wait_status: int) -> int:
    if os.WIFEXITED(wait_status):
        return os.WEXITSTATUS(wait_status)
    if os.WIFSIGNALED(wait_status):
        return 128 + os.WTERMSIG(wait_status)
    return 125


def kill_child_group(pid: int) -> None:
    """Kill the PTY session so timed-out grandchildren cannot survive."""
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        return


def main() -> int:
    args = parse_args()
    transcript_path = Path(args.transcript)
    status_path = Path(args.status_file)
    scripted_input = Path(args.input_file).read_bytes() if args.input_file else b""
    wait_for = args.wait_for.encode() if args.wait_for is not None else None
    transcript = bytearray()
    input_sent = False
    deadline = time.monotonic() + args.timeout

    pid, master_fd = pty.fork()
    if pid == 0:
        os.execvpe(args.command[0], args.command, os.environ.copy())

    try:
        while True:
            if not input_sent and (wait_for is None or wait_for in transcript):
                if scripted_input:
                    os.write(master_fd, scripted_input)
                if args.send_eof:
                    os.write(master_fd, b"\x04")
                input_sent = True

            child_pid, wait_status = os.waitpid(pid, os.WNOHANG)
            if child_pid == pid:
                while True:
                    try:
                        chunk = os.read(master_fd, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not chunk:
                        break
                    transcript.extend(chunk)
                transcript_path.write_bytes(transcript)
                status_path.write_text(f"{child_exit_code(wait_status)}\n", encoding="ascii")
                if wait_for is not None and not input_sent:
                    print("PTY child exited before the input marker appeared", file=sys.stderr)
                    return 126
                return 0

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                kill_child_group(pid)
                _, wait_status = os.waitpid(pid, 0)
                transcript_path.write_bytes(transcript)
                status_path.write_text(f"{child_exit_code(wait_status)}\n", encoding="ascii")
                print(f"PTY command timed out after {args.timeout:g} seconds", file=sys.stderr)
                return 124

            readable, _, _ = select.select([master_fd], [], [], min(0.05, remaining))
            if readable:
                try:
                    chunk = os.read(master_fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        continue
                    raise
                if chunk:
                    transcript.extend(chunk)
    finally:
        os.close(master_fd)


if __name__ == "__main__":
    raise SystemExit(main())
