// Koala's vendor-specific testharness.js reporter.
//
// Served by wptrunner at `/resources/testharnessreport.js` for
// any test run under `--product=koala`. The HTTP route override
// is configured via `env_options()["testharnessreport"]` in
// `koala.py`, mirroring the pattern used by Servo and WKTR
// (their `testharnessreport-servo.js` and
// `testharnessreport-wktr.js` files in
// `tools/wptrunner/wptrunner/`).
//
// Loads AFTER testharness.js and BEFORE any user `<script>`
// that calls `test()` / `async_test()` / `promise_test()`, per
// the standard WPT test layout:
//
//   <script src="/resources/testharness.js"></script>
//   <script src="/resources/testharnessreport.js"></script>
//   <script>test(function () { ... }, "name");</script>
//
// By the time any test runs, our `add_result_callback` is
// already registered and test completions fire straight into
// `__koala_emit_result__` (defined by `koala_wpt::install`).

(function () {
    // Pass `output: false` to suppress testharness.js's
    // DOM-rendering of results into `<div id="log">`. We capture
    // results out-of-band via the callbacks below — rendering
    // them is unnecessary work that exercises DOM bridge surface
    // koala may not implement.
    //
    // The other template args are the standard ones wptrunner
    // fills in for every browser; see
    // `tools/wptrunner/wptrunner/environment.py`'s
    // `testharnessreport_format_args`.
    setup({
        output: false,
        timeout_multiplier: %(timeout_multiplier)s,
        explicit_timeout: %(explicit_timeout)s,
        debug: %(debug)s
    });

    add_result_callback(function (test) {
        __koala_emit_result__(test);
    });

    add_completion_callback(function (tests, status) {
        __koala_emit_completion__(tests, status);
    });
})();
