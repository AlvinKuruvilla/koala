# Open the browser GUI. The address bar handles URL navigation
# after launch, so this recipe takes no argument.
#   just gui
gui:
    cargo run --bin koala-ui

# Run the headless CLI, optionally saving a screenshot.
#   just cli https://example.com
#   just cli res/test.html
#   just cli https://example.com screenshot.png
cli url screenshot="":
    @if [ -z "{{screenshot}}" ]; then \
        cargo run --bin koala -- "{{url}}"; \
    else \
        cargo run --bin koala -- -S "{{screenshot}}" "{{url}}"; \
    fi

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

# One build's raw `--bench` report as JSON on stdout: every measured load
# and render, heap accounting, and fingerprints of the input and the
# rendered frame. To compare builds, use `just lab compare`.
#
# TARGET is a recorded corpus page (replayed, no network; see
# `just lab corpus list`), a local file, or a URL (fetched live on every
# load; see `just bench-live`).
#
#   just bench                        # the landing page
#   just bench google                 # a recorded page
#   just bench res/test.html > out.json
#
# One build's raw timing report for a recorded page, file, or URL.
bench target="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(just lab corpus args "{{target}}")
    cargo run --release --features bench --bin koala -- \
        --bench "${args[@]}" --width 2048 --height 1536

# Same as `just bench` but for a live URL, including network and
# external-resource fetch cost. `--setup-iterations 1 --setup-warmup 0`
# because each load is a real fetch: 25 of them would hammer the server,
# and network variance swamps anything a repeat would average out.
#
#   just bench-live https://example.com
#
# One build's raw timing report for a live URL, network included.
bench-live url:
    cargo run --release --features bench --bin koala -- \
        --bench "{{url}}" --width 2048 --height 1536 \
        --setup-iterations 1 --setup-warmup 0

# Profile the render pipeline with `cargo flamegraph` and write
# `flamegraph.svg` (gitignored). TARGET is as for `just bench`. macOS
# needs `sudo` for dtrace; the flag prompts once. Requires
# `cargo install flamegraph`.
#
#   just flame                        # the landing page
#   just flame google                 # a recorded page, no network
#
# Flamegraph of a recorded page, file, or URL.
flame target="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(just lab corpus args "{{target}}")
    sudo cargo flamegraph --release --features bench --bin koala \
        -- --bench "${args[@]}" --bench-iterations 10 --bench-warmup 2 \
           --width 2048 --height 1536 > /dev/null
    echo "Flamegraph written to flamegraph.svg"

# Live counterpart of `just flame`, network and JS pump included.
# Iterations are capped at 1 because setup cost dominates; the aim is
# call-stack coverage of the load, not statistics.
#
#   just flame-live https://google.com
#
# Flamegraph of one live load, network included.
flame-live url:
    sudo cargo flamegraph --release --features bench --bin koala \
        -- --bench "{{url}}" --bench-iterations 1 --bench-warmup 0 \
           --width 2048 --height 1536 > /dev/null
    echo "Flamegraph written to flamegraph.svg"

# Per-stage allocation probe. Loads `url` through
# `koala_browser::load_document` with a `tracing` layer that
# reports peak resident memory at every span enter / exit; the
# stream of `enter`/`close` lines on stderr shows which pipeline
# stage is allocating. Stack `--map URL=PATH` overrides to swap
# fetched scripts / CSS for instrumented local copies.
#
#   just probe-oom https://example.com
#   just probe-oom https://example.com --map URL=/tmp/x.js
#   just probe-oom https://example.com 2> /tmp/trail.log
probe-oom url *MAPS:
    cargo run --release --bin oom-probe -- {{MAPS}} "{{url}}"

# Minimal-repro harness for "does this JS file misbehave in Boa
# on its own?" Runs each `<file>` through a single fresh
# `JsRuntime` against an empty DOM and reports wall-time +
# peak-RSS per script. Pass multiple files to test order /
# interaction effects.
#
#   just probe-boa tmp/script-12.js
#   just probe-boa tmp/scripts/script-0{0..7}.js
probe-boa +FILES:
    cargo run --release --bin boa-isolate -- {{FILES}}

# One-time setup for the WPT integration: creates `.venv-wpt/`,
# installs the koala wptrunner plugin, and pulls in wpt's Python
# requirements. Safe to re-run; pip will no-op when versions match.
#
# `blessings` is the colour backend mozterm/machformatter look for
# when deciding whether to colourise PASS / FAIL / TIMEOUT lines.
# wpt doesn't list it as a hard dependency — without it, every
# TEST_END line renders monochrome. Adding it here means `just
# wpt` produces coloured live output out of the box.
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
# Always writes a JSON wptreport to /tmp/koala-wpt.json so a
# directory run's output is analyzable after the fact (per-test
# status, subtest results, timing) without standing up the
# dashboard. Use `just wpt-record` instead if you want the run
# archived under `dashboard/runs/`.
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
wpt test="/css/CSS2/visudet/content-height-001.html" processes="4":
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
            --log-wptreport=/tmp/koala-wpt.json \
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
    .venv-wpt/bin/python -m wptrunner_koala.summary /tmp/koala-wpt.json
    exit "$rc"

# List the top-level WPT areas sorted by test-file count, with a
# hint on how to drive `just wpt` against one. Takes ~10-30s on
# first run (filesystem walk over ~900MB); fast afterward thanks
# to the OS file cache.
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

# Run wpt against `scope` and archive the JSON report into
# `dashboard/runs/<timestamp>_<sha>.json`. Use this when you want a
# run to land in the conformance dashboard. For one-off iteration use
# `just wpt` instead — it doesn't archive, so the runs/ dir doesn't
# fill with throwaway debug data.
#
# `processes` is the parallel koala-cli count; wptrunner shards the
# test list across that many subprocesses pulling from one wpt server.
# A good default is the physical core count; raise it if I/O-bound,
# lower it to debug. Each koala-cli writes its own hosts file and
# JSON-protocol streams independently, so they don't share state.
#
#   just wpt-record                                       # default scope, 4 processes
#   just wpt-record /css/CSS2/visudet/                    # whole subdir
#   just wpt-record /css/ 8                               # /css/ at 8x parallel
wpt-record scope="/css/CSS2/visudet/" processes="4":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release -p koala-cli
    mkdir -p dashboard/runs
    ts=$(date -u +%Y-%m-%dT%H-%M-%S)
    sha=$(git rev-parse --short HEAD)
    out="dashboard/runs/${ts}_${sha}.json"
    .venv-wpt/bin/python third-party/wpt/wpt \
        --venv .venv-wpt --skip-venv-setup \
        run \
            --binary="{{justfile_directory()}}/target/release/koala" \
            --processes="{{processes}}" \
            --no-pause \
            --no-restart-on-unexpected \
            --log-mach=- --log-mach-level=warning \
            --log-wptreport="$out" \
            koala "{{scope}}"
    echo "Archived run to $out"

# Install the dashboard's Node dependencies (Observable Framework)
# plus the Python deps the data loaders need (`duckdb` for the
# parquet emitter). The Python install lands in `.venv-wpt`, which
# `just wpt-setup` is responsible for creating.
#   just wpt-setup           # if you haven't already
#   just dashboard-setup
dashboard-setup:
    cd dashboard && npm install
    .venv-wpt/bin/pip install --quiet duckdb

# Build the static dashboard into dashboard/dist/. Observable's data
# loaders re-run on every build (they read dashboard/runs/), so the
# dashboard always reflects whatever runs are currently archived.
# We prepend `.venv-wpt/bin` to PATH so the `#!/usr/bin/env python3`
# shebang in our loaders picks up the wpt venv (which has `duckdb`)
# instead of a system python3 that probably doesn't.
dashboard-build:
    cd dashboard && PATH="{{justfile_directory()}}/.venv-wpt/bin:$PATH" npm run build

# Start the Observable preview server on http://127.0.0.1:3000 with
# hot reload. Edit src/*.md and the page re-renders automatically.
# Same PATH injection as `dashboard-build`.
dashboard-serve:
    cd dashboard && PATH="{{justfile_directory()}}/.venv-wpt/bin:$PATH" npm run dev

# Clear Observable's data-loader cache and the built site. Useful
# after changing the data loader's output schema (Observable caches
# loader output and may serve a stale shape otherwise).
dashboard-clean:
    rm -rf dashboard/src/.observablehq dashboard/.observablehq dashboard/dist

# Tear down the wpt venv and clean up any koala temp screenshots
# left behind by interrupted runs.
wpt-clean:
    rm -rf .venv-wpt
    find /tmp /var/folders -name 'koala-wpt-*.png' -delete 2>/dev/null || true

# Lint, type-check, and test the Python packages: the same commands the
# `python` CI workflow runs.
#
# Lint, type-check, and test the Python packages.
py-check:
    cd tools/koala-lab && uv run ruff check && uv run ruff format --check \
        && uv run mypy src tests && uv run pytest

# Measure koala builds against each other (koala-lab). Common uses:
#
#   just lab compare                 # working tree vs where this branch left master
#   just lab compare master my-branch
#   just lab show                    # re-print the latest result
#   just lab corpus record           # (re)record the pages measured
#
# Compare koala builds, record pages, show results (koala-lab).
lab *ARGS:
    @uv run --project {{justfile_directory()}}/tools/koala-lab koala-lab {{ARGS}}
