"""Executor-side glue for the koala wptrunner plugin.

Translates wptrunner's per-test ``do_test`` / ``screenshot`` calls
into the JSON-lines protocol that ``koala-cli --wpt-protocol``
speaks. Ships two executors: ``RefTestExecutor`` (Phase 1, pixel-
diff reftests) and ``TestharnessExecutor`` (Phase 5 chunk 3,
testharness.js result reporting → M2).
"""

import json
import os
from base64 import b64encode
from queue import Empty
from time import time

from wptrunner.executors.base import (
    RefTestExecutor,
    RefTestImplementation,
    TestharnessExecutor,
)
from wptrunner.executors.protocol import Protocol, ProtocolPart


# koala-js encodes testharness.js's `Test.status` enum as a
# numeric for transport over the JSON-lines protocol. Map back to
# wptrunner's string statuses on the way out. Unknown statuses
# fall through to "FAIL" so the surrounding tooling treats them
# as test failures rather than silently dropping the result.
_SUBTEST_STATUS_NAMES = {
    0: "PASS",
    1: "FAIL",
    2: "TIMEOUT",
    3: "NOTRUN",
    4: "PRECONDITION_FAILED",
}

# Same mapping for `TestsStatus.status` (the harness-level
# completion code). Unknown values fall through to "ERROR" so an
# unrecognised state shows up as a hard failure rather than a
# misleading pass.
_HARNESS_STATUS_NAMES = {
    0: "OK",
    1: "ERROR",
    2: "TIMEOUT",
    3: "PRECONDITION_FAILED",
}


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


class KoalaTestharnessPart(ProtocolPart):
    """Sends a single ``testharness`` command and waits for the
    matching ``testharness_complete`` event. Returns the parsed
    ``(harness_status, harness_message, subtests)`` triple in
    wptrunner's expected shape."""

    name = "koala_testharness"

    def __init__(self, parent):
        super().__init__(parent)
        self.stdin_queue = parent.browser.stdin_queue
        self.stdout_queue = parent.browser.stdout_queue
        self._ready_seen = False

    def wait_for_ready(self, timeout=10.0):
        """Consume koala-cli's startup ``ready`` event. Shares the
        ``_ready_seen`` latch shape with ``KoalaRenderPart`` — both
        kinds of run end up gating on the same handshake but each
        protocol part tracks it independently because wptrunner
        may instantiate them on separate test batches."""
        if self._ready_seen:
            return
        event = _read_event(self.stdout_queue, time() + timeout)
        if event.get("event") != "ready":
            raise KoalaProtocolError(f"expected ready event, got {event!r}")
        self._ready_seen = True

    def run(self, url, timeout):
        """Send a testharness command for ``url`` and return the
        decoded ``(status, message, subtests)`` triple.

        ``subtests`` is a list of ``(name, status, message, stack)``
        tuples in emission order, where ``status`` is one of the
        wptrunner subtest status strings (PASS, FAIL, TIMEOUT,
        NOTRUN, PRECONDITION_FAILED). ``status`` for the harness
        itself is similarly one of OK, ERROR, TIMEOUT,
        PRECONDITION_FAILED.

        Raises ``KoalaProtocolError`` for load failures /
        unrecognised events, and ``TimeoutError`` if ``timeout``
        elapses before ``testharness_complete`` arrives.
        """
        self.wait_for_ready()

        command = {"cmd": "testharness", "url": url}
        self.stdin_queue.put((json.dumps(command) + "\n").encode("utf-8"))

        deadline = time() + timeout if timeout else None
        while True:
            event = _read_event(self.stdout_queue, deadline)
            etype = event.get("event")
            if etype == "testharness_complete" and event.get("url") == url:
                return self._decode(event)
            if etype == "load_failed" and event.get("url") == url:
                raise KoalaProtocolError(
                    f"koala load_failed for {url}: {event.get('error')}"
                )
            if etype == "protocol_error":
                raise KoalaProtocolError(
                    f"koala protocol_error: {event.get('message')}"
                )
            self.logger.debug(f"ignoring stale event: {event!r}")

    def _decode(self, event):
        """Turn a ``testharness_complete`` event into the triple
        wptrunner's ``TestharnessExecutor`` expects.

        - When koala-cli reports a completion payload, use its
          status + message.
        - When the harness completion callback never fired (e.g.
          the document didn't include testharness.js), default
          to ``OK`` with an empty message. The per-test results
          (if any) still surface through the subtests list.
        """
        completion = event.get("completion")
        if completion is None:
            harness_status = "OK"
            harness_message = ""
        else:
            harness_status = _HARNESS_STATUS_NAMES.get(
                completion.get("status"), "ERROR"
            )
            harness_message = completion.get("message", "")

        subtests = []
        for raw in event.get("results", []):
            subtest_status = _SUBTEST_STATUS_NAMES.get(
                raw.get("status"), "FAIL"
            )
            subtests.append((
                raw.get("name", ""),
                subtest_status,
                raw.get("message", ""),
                raw.get("stack", ""),
            ))
        return harness_status, harness_message, subtests


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
    implements = [KoalaRenderPart, KoalaTestharnessPart, KoalaErrorsPart]

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


class KoalaTestharnessExecutor(TestharnessExecutor):
    """Runs a testharness.js test through koala-cli's
    ``testharness`` protocol command and converts the captured
    result frame into wptrunner's ``TestharnessResult`` shape.

    Mirrors ``KoalaRefTestExecutor``'s error-handling pattern:
    protocol errors and timeouts surface as ERROR /
    EXTERNAL-TIMEOUT harness statuses with any captured stderr
    appended to the message so the wptrunner log is useful when
    things go wrong.
    """

    def __init__(self, logger, browser, server_config, timeout_multiplier=1,
                 debug_info=None, **kwargs):
        super().__init__(
            logger, browser, server_config, timeout_multiplier,
            debug_info, **kwargs,
        )
        self.protocol = KoalaProtocol(self, browser)

    def do_test(self, test):
        url = self.test_url(test)
        timeout = test.timeout * self.timeout_multiplier
        try:
            harness_status, harness_message, subtests = (
                self.protocol.koala_testharness.run(url, timeout)
            )
        except KoalaProtocolError as exc:
            errors = self.protocol.koala_errors.read_errors()
            return (
                test.make_result("ERROR", f"{exc}\n{errors}"),
                [],
            )
        except TimeoutError:
            errors = self.protocol.koala_errors.read_errors()
            return (test.make_result("EXTERNAL-TIMEOUT", errors), [])

        subtest_results = [
            test.make_subtest_result(name, status, message, stack)
            for (name, status, message, stack) in subtests
        ]
        return (
            test.make_result(harness_status, harness_message),
            subtest_results,
        )
