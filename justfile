# Recipes are grouped by task. A recipe earns its place by being a daily
# entry point or by encoding something that would otherwise have to be
# rediscovered (flags, environment, paths); a variant of an existing task
# is an argument, not a new recipe.

# Debug builds of the GUI and CLI run under lldb in batch mode. A clean
# run exits with the program's own status; a crash stops, prints the
# innermost 60 frames of every thread, and exits 134, with no need to
# reproduce it under a debugger by hand. Rust's own handler prints no
# backtrace for a stack overflow, which is when this matters most.
lldb_run := "lldb --batch --no-lldbinit -o run -o 'script import os; os._exit(lldb.process.GetExitStatus())' -k 'thread backtrace all -c 60' -k 'script import os; os._exit(134)' --"

# Open the browser GUI (debug build, under lldb). The address bar
# handles navigation, so this takes no argument.
[doc("Open the browser GUI under lldb (a crash prints a backtrace)")]
gui:
    cargo build --bin koala-ui
    {{lldb_run}} target/debug/koala-ui

# Load TARGET in the headless CLI (debug build, under lldb) and print its
# DOM, or save a screenshot. TARGET is a recorded corpus page (replayed;
# see `just lab corpus list`), a local file, or a URL.
#
#   just cli https://example.com
#   just cli google screenshot.png
[doc("Load a page in the headless CLI under lldb; optionally screenshot it")]
cli target screenshot="":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --bin koala
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(just lab corpus args "{{target}}")
    if [ -n "{{screenshot}}" ]; then args=(-S "{{screenshot}}" "${args[@]}"); fi
    {{lldb_run}} target/debug/koala "${args[@]}"

# Fetch a page and pretty-print it with Prettier, expanding any
# minified embedded <style>/<script> blocks into readable, indented
# code. Handy for eyeballing a real page's markup + CSS or for
# seeding a hand-edited test fixture from a live site.
#
# Output defaults to `tmp/prettify/<slug>.html` (gitignored scratch);
# pass a second argument to write elsewhere (e.g. a res/ fixture).
# The path is echoed on success.
#
# Prettier runs via `npx --yes prettier@3`, so only Node/npx need to
# be on PATH — no global Prettier install required.
#
#   just prettify https://discord.com
#   just prettify https://discord.com res/fixtures/discord.html
[doc("Fetch a page and pretty-print its HTML for reading or as a fixture")]
prettify url out="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! "{{url}}" =~ ^https?:// ]]; then
        echo "error: expected an http(s) URL, got '{{url}}'" >&2
        exit 1
    fi
    out="{{out}}"
    if [ -z "$out" ]; then
        slug=$(echo "{{url}}" | sed 's|https*://||; s|[^a-zA-Z0-9]|_|g')
        out="tmp/prettify/${slug}.html"
    fi
    mkdir -p "$(dirname "$out")"
    # Fetch to a temp first so a failed download never leaves a
    # half-written or stale file at the destination. A desktop UA
    # keeps sites from serving stripped-down no-JS markup.
    raw=$(mktemp)
    trap 'rm -f "$raw"' EXIT
    curl -sL --fail --max-time 30 \
        -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" \
        "{{url}}" -o "$raw"
    npx --yes prettier@3 --parser html "$raw" > "$out"
    echo "Wrote $out ($(wc -l < "$out" | tr -d ' ') lines)"

# One build's raw timing report as JSON on stdout: every measured load
# and render, heap accounting, and fingerprints of the input and the
# rendered frame. To compare builds, use `just lab compare`.
#
# TARGET is a recorded corpus page (replayed, no network), a local file,
# or a URL. A URL is fetched live on every load, so it gets one load and
# no warm-up: repeats would hammer the server, and network variance
# swamps anything they would average out.
#
#   just bench                        # the landing page
#   just bench google                 # a recorded page
#   just bench https://example.com    # live, network included
[doc("One build's raw timing report for a recorded page, file, or URL")]
bench target="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(just lab corpus args "{{target}}")
    live=()
    if [[ "${args[0]}" =~ ^https?:// ]]; then live=(--setup-iterations 1 --setup-warmup 0); fi
    cargo run --release --features bench --bin koala -- \
        --bench "${args[@]}" --width 2048 --height 1536 ${live[@]+"${live[@]}"}

# Flamegraph of TARGET (as for `just bench`) written to flamegraph.svg
# (gitignored). A live URL gets one load and one render: the aim is
# call-stack coverage, not statistics. macOS needs `sudo` for dtrace.
# Requires `cargo install flamegraph`.
#
#   just flame google
[doc("Flamegraph of a recorded page, file, or URL")]
flame target="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(just lab corpus args "{{target}}")
    counts=(--bench-iterations 10 --bench-warmup 2)
    if [[ "${args[0]}" =~ ^https?:// ]]; then
        counts=(--bench-iterations 1 --bench-warmup 0 --setup-iterations 1 --setup-warmup 0)
    fi
    sudo cargo flamegraph --release --features bench --bin koala \
        -- --bench "${args[@]}" "${counts[@]}" --width 2048 --height 1536 > /dev/null
    echo "Flamegraph written to flamegraph.svg"

# Measure koala builds against each other (koala-lab).
#
#   just lab compare                 # working tree vs where this branch left master
#   just lab compare master my-branch
#   just lab show                    # re-print the latest result
#   just lab corpus record           # (re)record the pages measured
[doc("Compare koala builds on the corpus (koala-lab)")]
lab *ARGS:
    @uv run --project {{justfile_directory()}}/tools/koala-lab koala-lab {{ARGS}}

# Minimal-repro harness for "does this JS file misbehave in Boa
# on its own?" Runs each `<file>` through a single fresh
# `JsRuntime` against an empty DOM and reports wall-time +
# peak-RSS per script. Pass multiple files to test order /
# interaction effects.
#
#   just probe-boa tmp/script-12.js
#   just probe-boa tmp/scripts/script-0{0..7}.js
[doc("Run JS files through a fresh Boa runtime; time and peak memory each")]
probe-boa +FILES:
    cargo run --release --bin boa-isolate -- {{FILES}}

# Lint, type-check, and test the Python packages, as the `python` CI
# workflow does.
[doc("Lint, type-check, and test the Python packages (as CI does)")]
py-check:
    cd tools/koala-lab && uv run ruff check && uv run ruff format --check \
        && uv run mypy src tests && uv run pytest

# One-time setup for the WPT integration: creates `.venv-wpt/`,
# installs the koala wptrunner plugin, and pulls in wpt's Python
# requirements. Safe to re-run; pip will no-op when versions match.
#
# `blessings` is the colour backend mozterm/machformatter look for
# when deciding whether to colourise PASS / FAIL / TIMEOUT lines.
# wpt doesn't list it as a hard dependency — without it, every
# TEST_END line renders monochrome. Adding it here means `just
# wpt` produces coloured live output out of the box.
[doc("Create .venv-wpt with the wptrunner plugin and wpt's requirements")]
wpt-setup:
    python3 -m venv .venv-wpt
    .venv-wpt/bin/pip install --upgrade pip
    .venv-wpt/bin/pip install -e wpt-tools/wptrunner-koala
    .venv-wpt/bin/pip install -r third-party/wpt/tools/wptrunner/requirements.txt
    .venv-wpt/bin/pip install blessings

# Run a WPT test (or directory) against koala via the wpt-protocol
# plugin. Builds koala-cli in release mode first (incremental, so
# no-op when up to date). The first invocation downloads the WPT
# manifest (~40MB).
#
# Always writes a JSON wptreport, so a directory run's output is
# analyzable after the fact (per-test status, subtest results, timing).
# It goes to /tmp/koala-wpt.json, or with `record` to
# `dashboard/runs/<timestamp>_<sha>.json`, where the conformance
# dashboard reads it. Record only runs worth keeping: one-off debugging
# runs would fill the dashboard with noise.
#
# `processes` is the parallel koala-cli count. The current
# rate-limiter on every directory run is tests that hit wpt's
# 10s per-test deadline (because koala doesn't implement enough
# DOM yet for testharness.js to declare any subtests), so a
# directory's wall time is dominated by N timeout tests × 10s.
# Parallelism cuts that linearly. Default is 4, matching
# wpt-record; pass `1` for clean sequential output (single-test
# debugging) or higher to push more cores. Each koala-cli writes
# its own JSON-protocol stream and they don't share state.
#
#   just wpt                                                # smoke test
#   just wpt /css/CSS2/visudet/content-height-001.html      # single test
#   just wpt /css/CSS2/visudet/content-height-001.html 1    # force serial
#   just wpt /dom/nodes/                                    # whole dir, 4 parallel
#   just wpt /dom/nodes/ 8                                  # whole dir, 8 parallel
#   just wpt /css/CSS2/visudet/ 4 record                    # archive for the dashboard
[doc("Run WPT tests against koala; `record` archives the run for the dashboard")]
wpt test="/css/CSS2/visudet/content-height-001.html" processes="4" record="":
    #!/usr/bin/env bash
    # Shebang form so we own the whole script and can:
    #   1. Keep going past `wpt run` exiting non-zero (it does
    #      whenever any test has an unexpected result — default
    #      just behaviour would skip the summary exactly when
    #      it matters most).
    #   2. Hold off on the koala summary until wpt's wptserve
    #      subprocesses have finished flushing their shutdown
    #      log lines, so the summary lands at the very bottom
    #      rather than mid-shutdown.
    #   3. Propagate wpt's exit code back to just / CI.
    set -uo pipefail
    cargo build --release -p koala-cli
    report=/tmp/koala-wpt.json
    if [ "{{record}}" = record ]; then
        mkdir -p dashboard/runs
        report="dashboard/runs/$(date -u +%Y-%m-%dT%H-%M-%S)_$(git rev-parse --short HEAD).json"
    elif [ -n "{{record}}" ]; then
        echo "error: third argument must be 'record' or empty, got '{{record}}'" >&2
        exit 2
    fi
    # PYTHONWARNINGS silences wpt-pinned urllib3 v2's
    # `NotOpenSSLWarning` (Python 3.9 on macOS links against
    # LibreSSL, not OpenSSL). The warning is informational and
    # not actionable from our side — wpt's requirements.txt pins
    # urllib3 to exactly 2.6.3.
    #
    # Filter by message text rather than category class: -W
    # parses before site.py runs, so `urllib3.exceptions` isn't
    # importable yet and a category-based filter is rejected
    # with "invalid module name". The message form matches a
    # regex against the warning's start, which Python can
    # evaluate without importing anything.
    rc=0
    PYTHONWARNINGS="ignore:urllib3 v2 only supports OpenSSL" \
    .venv-wpt/bin/python third-party/wpt/wpt \
        --venv .venv-wpt --skip-venv-setup \
        run \
            --binary="{{justfile_directory()}}/target/release/koala" \
            --processes="{{processes}}" \
            --no-pause \
            --no-restart-on-unexpected \
            --log-mach=- --log-mach-level=info \
            --log-wptreport="$report" \
            koala "{{test}}" || rc=$?
    # `wpt run` returns once its main thread is done, but its
    # wptserve worker subprocesses keep emitting "Stopped http
    # server" / "Closing logging queue" lines for a moment
    # afterwards (separate processes, inherited stdout, no way
    # for bash to `wait` on them). Pause long enough that those
    # stragglers land before the summary; 500ms is well past
    # the observed shutdown noise without being noticeable.
    sleep 0.5
    echo
    .venv-wpt/bin/python -m wptrunner_koala.summary "$report"
    if [ "{{record}}" = record ]; then echo "Archived run to $report"; fi
    exit "$rc"

# List the top-level WPT areas sorted by test-file count, with a
# hint on how to drive `just wpt` against one. Takes ~10-30s on
# first run (filesystem walk over ~900MB); fast afterward thanks
# to the OS file cache.
[doc("List WPT areas by test count")]
wpt-list:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d third-party/wpt ]; then
        echo "WPT submodule not initialized. Run: git submodule update --init --recursive" >&2
        exit 1
    fi
    cd third-party/wpt
    {
        for d in */; do
            name="${d%/}"
            count=$(find "$d" -type f \( -name "*.html" -o -name "*.xht" -o -name "*.xhtml" \) 2>/dev/null | wc -l | tr -d ' ')
            [ "$count" -gt 0 ] || continue
            printf "  /%-40s %8d\n" "${name}/" "$count"
        done
    } | sort -k2 -rn
    echo
    echo "Run a family with:  just wpt /<area>/[<subdir>/]"

# The conformance dashboard (Observable Framework) over the runs archived
# by `just wpt ... record`:
#
# - setup: Node dependencies, plus `duckdb` for the parquet loader, into
#   `.venv-wpt` (run `just wpt-setup` first).
# - serve: preview on http://127.0.0.1:3000 with hot reload.
# - build: static site into dashboard/dist/; loaders re-read the runs.
# - clean: drop Observable's loader cache and the built site, needed
#   after changing a loader's output shape (a stale shape may be served).
#
# `.venv-wpt/bin` goes first on PATH so the loaders' `#!/usr/bin/env
# python3` shebang finds the venv's `duckdb`, not a system python3.
#
#   just dashboard setup
#   just dashboard
[doc("Conformance dashboard: setup, serve (default), build, or clean")]
dashboard action="serve":
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="{{justfile_directory()}}/.venv-wpt/bin:$PATH"
    case "{{action}}" in
        setup) (cd dashboard && npm install) && pip install --quiet duckdb ;;
        serve) cd dashboard && npm run dev ;;
        build) cd dashboard && npm run build ;;
        clean) rm -rf dashboard/src/.observablehq dashboard/.observablehq dashboard/dist ;;
        *) echo "error: action must be setup, serve, build, or clean; got '{{action}}'" >&2; exit 2 ;;
    esac
