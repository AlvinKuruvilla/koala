"""Browser-side glue for the koala wptrunner plugin.

Spawns ``koala-cli --wpt-protocol`` as a subprocess and exposes its
stdin/stdout/stderr to the executor via the standard wptrunner
``ExecutorBrowser`` queue pattern (modelled after ``wktr.py``).

Loaded indirectly via :func:`wptrunner_koala.load`, which builds a
wptrunner ``Product`` from the ``__wptrunner__`` dict at the bottom
of this file.
"""

import os
import subprocess
import tempfile
import threading
from multiprocessing import Queue

from wptrunner.browsers.base import Browser, ExecutorBrowser
from wptrunner.executors import executor_kwargs as base_executor_kwargs

# Re-exports for `__wptrunner__` discovery. wptrunner's
# `Product._from_dunder_wptrunner` resolves each string value in the
# dict via `getattr(module, name)`, so every name referenced from
# `__wptrunner__` must be live in this module's namespace even if
# nothing here references it directly.
from wptrunner.browsers.base import get_timeout_multiplier  # noqa: F401
from .executorkoala import (  # noqa: F401
    KoalaRefTestExecutor,
    KoalaTestharnessExecutor,
)


# Graceful-shutdown timeouts. After requesting `{"cmd":"shutdown"}`
# we wait `_SHUTDOWN_TIMEOUT_S` for the subprocess to exit on its
# own; if it doesn't, SIGKILL with another `_KILL_TIMEOUT_S` grace
# period. Reader/writer threads are joined for `_THREAD_JOIN_S`.
_SHUTDOWN_TIMEOUT_S = 5
_KILL_TIMEOUT_S = 2
_THREAD_JOIN_S = 2


def _make_hosts_file_text(server_config):
    """Lazy import of ``tools.serve.serve.make_hosts_file``.

    The ``tools.*`` package only lives on ``sys.path`` once the wpt
    CLI has run its ``localpaths`` shim — which happens before any
    product is started but after this module is imported for
    entry-point enumeration. Defer the import so plain imports of
    this module never raise.
    """
    from tools.serve.serve import make_hosts_file

    return make_hosts_file(server_config, "127.0.0.1")


def check_args(**kwargs):
    """``--binary`` is the only required argument: the path to ``koala-cli``."""
    if not kwargs.get("binary"):
        raise ValueError(
            "--binary=/path/to/koala-cli is required for --product=koala"
        )


def browser_kwargs(logger, test_type, run_info_data, config, subsuite, **kwargs):
    """Hand the binary path and the live server config (needed for
    hosts-file generation) to ``KoalaBrowser.__init__``."""
    return {
        "binary": kwargs["binary"],
        "binary_args": list(kwargs.get("binary_args") or []),
        "server_config": config,
    }


def executor_kwargs(logger, test_type, test_environment, run_info_data,
                    **kwargs):
    """Pass-through; koala has no driver capabilities to negotiate yet."""
    rv = base_executor_kwargs(test_type, test_environment, run_info_data,
                              **kwargs)
    rv["capabilities"] = {}
    return rv


def env_extras(**kwargs):
    """No extra processes or fixtures alongside koala."""
    return []


def env_options():
    """Server config + per-product overrides for the WPT environment.

    - HTTP-only on 127.0.0.1; no debugger.
    - `testharnessreport` overrides wptrunner's HTTP route for
      `/resources/testharnessreport.js` so that testharness.js
      results flow into koala's `__koala_emit_result__` /
      `__koala_emit_completion__` capture functions instead of
      vanishing into the upstream no-op stub. wptrunner resolves
      relative entries against its own module dir and leaves
      absolute paths alone (`tools/wptrunner/wptrunner/environment.py`
      → `os.path.join(here, path)`); we pass an absolute path so
      our file can live alongside this plugin.
    """
    here = os.path.dirname(os.path.abspath(__file__))
    return {
        "server_host": "127.0.0.1",
        "bind_address": True,
        "supports_debugger": False,
        "testharnessreport": [
            os.path.join(here, "testharnessreport-koala.js"),
        ],
    }


def update_properties():
    """Manifest-update properties tracked for koala results."""
    return (
        ["debug", "os", "processor"],
        {"os": ["version"], "processor": ["bits"]},
    )


class KoalaBrowser(Browser):
    """Wraps one ``koala-cli --wpt-protocol`` subprocess across a
    whole test batch. Each test sends one ``render`` command and
    waits for one response event matched by URL.
    """

    def __init__(self, logger, binary, binary_args=None, server_config=None,
                 **kwargs):
        super().__init__(logger, **kwargs)
        self._binary = binary
        self._binary_args = list(binary_args or [])
        self._server_config = server_config

        self._hosts_file_path = None
        self._proc = None
        self._stdout_queue = None
        self._stderr_queue = None
        self._stdin_queue = None
        self._readers = []

    def start(self, group_metadata, **kwargs):
        self._write_hosts_file()
        self._spawn_subprocess()
        self._start_io_threads()

    def stop(self, force=False):
        if not self.is_alive():
            self._cleanup_hosts_file()
            return True
        self._request_shutdown()
        self._wait_or_kill()
        self._join_io_threads()
        self._cleanup_hosts_file()
        return True

    def is_alive(self):
        return self._proc is not None and self._proc.poll() is None

    @property
    def pid(self):
        return self._proc.pid if self._proc else None

    def check_crash(self, process, test):
        return not self.is_alive()

    def executor_browser(self):
        return ExecutorBrowser, {
            "stdout_queue": self._stdout_queue,
            "stderr_queue": self._stderr_queue,
            "stdin_queue": self._stdin_queue,
        }

    def _write_hosts_file(self):
        if self._server_config is None:
            return
        fd, self._hosts_file_path = tempfile.mkstemp(
            prefix="koala-wpt-hosts-", suffix=".txt"
        )
        with os.fdopen(fd, "w") as fh:
            fh.write(_make_hosts_file_text(self._server_config))

    def _cleanup_hosts_file(self):
        if self._hosts_file_path is None:
            return
        try:
            os.remove(self._hosts_file_path)
        except OSError:
            pass
        self._hosts_file_path = None

    def _spawn_subprocess(self):
        cmd = [self._binary, "--wpt-protocol"]
        if self._hosts_file_path is not None:
            cmd += ["--hosts-file", self._hosts_file_path]
        cmd += self._binary_args
        self.logger.debug(f"Starting koala-cli: {' '.join(cmd)}")
        # Unbuffered: koala-cli flushes after every JSON event; binary
        # mode warns on line-buffered (bufsize=1).
        self._proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=os.environ.copy(),
            bufsize=0,
        )

    def _start_io_threads(self):
        self._stdout_queue = Queue()
        self._stderr_queue = Queue()
        self._stdin_queue = Queue()
        self._readers = [
            _spawn_reader(self._proc.stdout, self._stdout_queue),
            _spawn_reader(self._proc.stderr, self._stderr_queue),
            _spawn_writer(self._proc.stdin, self._stdin_queue),
        ]

    def _request_shutdown(self):
        try:
            self._stdin_queue.put(b'{"cmd":"shutdown"}\n')
        except (BrokenPipeError, ValueError):
            pass

    def _wait_or_kill(self):
        try:
            self._proc.wait(timeout=_SHUTDOWN_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            self._proc.kill()
            self._proc.wait(timeout=_KILL_TIMEOUT_S)

    def _join_io_threads(self):
        # Sentinel for the writer thread; readers stop on EOF.
        try:
            self._stdin_queue.put(None)
        except ValueError:
            pass
        for thread in self._readers:
            thread.join(_THREAD_JOIN_S)
        self._readers = []
        self._proc = None


def _spawn_reader(stream, queue):
    def reader():
        try:
            for line in iter(stream.readline, b""):
                queue.put(line)
        finally:
            stream.close()
    t = threading.Thread(target=reader, daemon=True)
    t.start()
    return t


def _spawn_writer(stream, queue):
    def writer():
        while True:
            item = queue.get()
            if item is None:
                break
            try:
                stream.write(item)
                stream.flush()
            except (BrokenPipeError, ValueError):
                break
        try:
            stream.close()
        except (BrokenPipeError, ValueError):
            pass
    t = threading.Thread(target=writer, daemon=True)
    t.start()
    return t


# Discovered by Product._from_dunder_wptrunner during entry-point load.
__wptrunner__ = {
    "product": "koala",
    "check_args": "check_args",
    "browser": "KoalaBrowser",
    "executor": {
        "reftest": "KoalaRefTestExecutor",
        "testharness": "KoalaTestharnessExecutor",
    },
    "browser_kwargs": "browser_kwargs",
    "executor_kwargs": "executor_kwargs",
    "env_extras": "env_extras",
    "env_options": "env_options",
    "timeout_multiplier": "get_timeout_multiplier",
    "update_properties": "update_properties",
}
