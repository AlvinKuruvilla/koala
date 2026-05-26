"""Executor-side glue for the koala wptrunner plugin.

Translates wptrunner's per-test ``do_test`` / ``screenshot`` calls
into the JSON-lines protocol that ``koala-cli --wpt-protocol``
speaks. For Phase 1 we only ship a ``RefTestExecutor``; testharness
support waits on the DOM bridge (see
``project-memory/wpt-integration-spec.md`` Phases 2–5).
"""

import json
import os
from base64 import b64encode
from queue import Empty
from time import time

from wptrunner.executors.base import (
    RefTestExecutor,
    RefTestImplementation,
)
from wptrunner.executors.protocol import Protocol, ProtocolPart


class KoalaProtocolError(Exception):
    """Raised when koala-cli emits a protocol-level error or an
    unrecoverable load failure during a render request."""


def _read_event(queue, deadline):
    """Block-read the next non-blank JSON event from the stdout queue.

    Raises ``TimeoutError`` if ``deadline`` is reached before one
    arrives, or ``KoalaProtocolError`` for malformed lines / closed
    streams. Blank lines (rare, but cheap to defend against) are
    skipped without recursion.
    """
    while True:
        now = time()
        if deadline is not None and now >= deadline:
            raise TimeoutError("deadline already past")

        try:
            line = queue.get(True, deadline - now if deadline else None)
        except Empty as exc:
            raise TimeoutError("no protocol event before deadline") from exc

        if not line:
            raise KoalaProtocolError("koala-cli closed stdout unexpectedly")

        try:
            text = line.decode("utf-8").strip()
        except UnicodeDecodeError as exc:
            raise KoalaProtocolError(
                f"non-utf8 protocol line: {line!r}"
            ) from exc

        if not text:
            continue

        try:
            return json.loads(text)
        except json.JSONDecodeError as exc:
            raise KoalaProtocolError(
                f"protocol channel returned non-JSON line: {text!r}"
            ) from exc


class KoalaRenderPart(ProtocolPart):
    """Sends a single ``render`` command and waits for the matching
    ``rendered`` / ``load_failed`` event. ``ready`` events are
    consumed once at startup and again any time we restart the
    subprocess between tests."""

    name = "koala_render"

    def __init__(self, parent):
        super().__init__(parent)
        self.stdin_queue = parent.browser.stdin_queue
        self.stdout_queue = parent.browser.stdout_queue
        self._ready_seen = False

    def wait_for_ready(self, timeout=10.0):
        """Consume the single ``ready`` event koala-cli emits at
        startup. Called once before the first render; idempotent."""
        if self._ready_seen:
            return
        event = _read_event(self.stdout_queue, time() + timeout)
        if event.get("event") != "ready":
            raise KoalaProtocolError(f"expected ready event, got {event!r}")
        self._ready_seen = True

    def render(self, url, viewport, timeout):
        """Send a render request and return the path to the screenshot
        PNG on success. Raises on failure."""
        self.wait_for_ready()

        command = {"cmd": "render", "url": url, "viewport": list(viewport)}
        self.stdin_queue.put((json.dumps(command) + "\n").encode("utf-8"))

        deadline = time() + timeout if timeout else None
        while True:
            event = _read_event(self.stdout_queue, deadline)
            etype = event.get("event")
            if etype == "rendered" and event.get("url") == url:
                return event["screenshot"]
            if etype == "load_failed" and event.get("url") == url:
                raise KoalaProtocolError(
                    f"koala load_failed for {url}: {event.get('error')}"
                )
            if etype == "protocol_error":
                raise KoalaProtocolError(
                    f"koala protocol_error: {event.get('message')}"
                )
            self.logger.debug(f"ignoring stale event: {event!r}")


class KoalaErrorsPart(ProtocolPart):
    """Drains stderr non-blockingly so the subprocess never deadlocks
    on a full pipe buffer, and so the executor can include any
    diagnostics in failure messages."""

    name = "koala_errors"

    def __init__(self, parent):
        super().__init__(parent)
        self.stderr_queue = parent.browser.stderr_queue

    def read_errors(self):
        chunks = []
        while not self.stderr_queue.empty():
            try:
                line = self.stderr_queue.get_nowait()
            except Empty:
                break
            chunks.append(line.decode("utf-8", errors="replace"))
        return "".join(chunks)


class KoalaProtocol(Protocol):
    implements = [KoalaRenderPart, KoalaErrorsPart]

    def connect(self):
        # No connection — the executor talks directly to the
        # subprocess via queues set up by KoalaBrowser.start().
        pass

    def after_connect(self):
        pass

    def teardown(self):
        pass

    def is_alive(self):
        return self.browser.is_alive() if hasattr(self.browser, "is_alive") else True


class KoalaRefTestExecutor(RefTestExecutor):
    """Renders the test and the ref via koala and hands the
    base64-encoded PNGs to wptrunner's reftest pixel-diff
    machinery."""

    def __init__(self, logger, browser, server_config, timeout_multiplier=1,
                 screenshot_cache=None, debug_info=None,
                 reftest_screenshot="unexpected", **kwargs):
        super().__init__(
            logger, browser, server_config, timeout_multiplier,
            screenshot_cache, debug_info, reftest_screenshot, **kwargs,
        )
        self.implementation = RefTestImplementation(self)
        self.protocol = KoalaProtocol(self, browser)

    def reset(self):
        self.implementation.reset()

    def do_test(self, test):
        try:
            result = self.implementation.run_test(test)
            return self.convert_result(test, result)
        except KoalaProtocolError as exc:
            errors = self.protocol.koala_errors.read_errors()
            return (
                test.make_result("ERROR", f"{exc}\n{errors}"),
                [],
            )
        except TimeoutError:
            errors = self.protocol.koala_errors.read_errors()
            return (test.make_result("EXTERNAL-TIMEOUT", errors), [])

    def screenshot(self, test, viewport_size, dpi, page_ranges):
        """wptrunner calls this once for the test and once for the
        ref. Both go through the same ``render`` protocol command."""
        assert dpi is None, "DPI override is not implemented in Phase 1"
        assert not self.is_print, "print-reftest is not implemented in Phase 1"

        viewport = viewport_size if viewport_size else (800, 600)
        url = self.test_url(test)
        timeout = test.timeout * self.timeout_multiplier

        try:
            screenshot_path = self.protocol.koala_render.render(
                url, viewport, timeout,
            )
        except KoalaProtocolError as exc:
            errors = self.protocol.koala_errors.read_errors()
            return False, ("ERROR", f"{exc}\n{errors}")
        except TimeoutError:
            return False, ("EXTERNAL-TIMEOUT", self.protocol.koala_errors.read_errors())

        try:
            with open(screenshot_path, "rb") as fh:
                png = fh.read()
        except OSError as exc:
            return False, ("ERROR", f"could not read screenshot {screenshot_path}: {exc}")
        finally:
            # koala-cli wrote to a temp path it owns; clean up so a
            # long-running batch doesn't fill /tmp.
            try:
                os.remove(screenshot_path)
            except OSError:
                pass

        return True, b64encode(png).decode("ascii")

    def wait(self):
        return
