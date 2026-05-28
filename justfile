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

# Bench the render pipeline against `url` (file path or HTTP URL)
# and emit a JSON timing report on stdout. URLs auto-cache to
# `.bench-cache/<slug>.html` on first use so subsequent runs are
# decoupled from network / page-content drift; refresh with
# `just bench-refresh`. File paths bypass the cache and are used
# directly (so the bundled landing page always runs against the
# current `res/landing.html`).
#
# Always uses `--features bench` (which transitively enables
# `koala-browser/render-trace`) and `--release` (so optimized code
# is what gets measured). Per-stage timings live in the `render`
# section of the JSON; `setup_us` is the one-time load cost.
#
#   just bench                                  # bench landing page
#   just bench https://example.com              # bench live URL (auto-cached)
#   just bench res/test.html                    # bench a local file
#   just bench .bench-cache/google_com.html     # bench a manually-named snapshot
#   just bench https://example.com > out.json   # capture for diffing
bench url="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{url}}" =~ ^https?:// ]]; then
        slug=$(echo "{{url}}" | sed 's|https*://||; s|[^a-zA-Z0-9]|_|g')
        target=".bench-cache/${slug}.html"
        if [ ! -f "$target" ]; then
            mkdir -p .bench-cache
            curl -sL "{{url}}" > "$target"
        fi
    else
        target="{{url}}"
    fi
    # 2048×1536 matches the koala-ui default window (1024×768
    # logical) at 2× retina. Fixed for reproducibility across
    # machines; edit here if a different shape becomes the
    # canonical comparison target.
    cargo run --release --features bench --bin koala -- \
        --bench "$target" --width 2048 --height 1536

# Same as `just bench` but skips the cache and passes the URL
# straight through, so `koala-cli` fetches it live. The harness
# still loads once and renders N times — only the one-time setup
# cost includes network I/O. The diff between `setup_us` from
# `just bench-live` vs `just bench` against the same URL is the
# end-to-end network + external-resource fetch cost (HTML,
# external CSS, external scripts, images, fonts).
#
# Use when you want realistic first-paint numbers and accept the
# network variance; use `just bench` when you want repeatable
# engine-only numbers across runs.
#
#   just bench-live https://google.com
#   just bench-live https://example.com > /tmp/live.json
bench-live url:
    cargo run --release --features bench --bin koala -- \
        --bench "{{url}}" --width 2048 --height 1536

# Re-fetch a remote URL into `.bench-cache/` so the next `just bench`
# against it sees fresh content. No-op for file paths (they're
# never cached).
#
#   just bench-refresh https://example.com
bench-refresh url:
    #!/usr/bin/env bash
    set -euo pipefail
    slug=$(echo "{{url}}" | sed 's|https*://||; s|[^a-zA-Z0-9]|_|g')
    mkdir -p .bench-cache
    curl -sL "{{url}}" > ".bench-cache/${slug}.html"
    echo "Refreshed .bench-cache/${slug}.html"

# Profile a render with `cargo flamegraph` and open the resulting
# SVG. macOS needs `sudo` for dtrace; the flag prompts once. Runs
# with `--features bench --release`, the same configuration as
# `just bench`, so the profile matches what the bench harness
# measures. Output lands in `flamegraph.svg` (gitignored).
#
# Requires `cargo install flamegraph` once.
#
#   just flame                          # profile landing page
#   just flame https://example.com      # profile live URL (auto-cached)
#   just flame res/test.html            # profile a local file
flame url="koala-ui/res/landing.html":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{url}}" =~ ^https?:// ]]; then
        slug=$(echo "{{url}}" | sed 's|https*://||; s|[^a-zA-Z0-9]|_|g')
        target=".bench-cache/${slug}.html"
        if [ ! -f "$target" ]; then
            mkdir -p .bench-cache
            curl -sL "{{url}}" > "$target"
        fi
    else
        target="{{url}}"
    fi
    sudo cargo flamegraph --release --features bench --bin koala \
        -- --bench "$target" --bench-iterations 10 --bench-warmup 2 \
           --width 2048 --height 1536 > /dev/null
    echo "Flamegraph written to flamegraph.svg"

# Live counterpart of `just flame` — profiles a URL with the full
# network + JS pipeline included (no caching). Iterations are
# capped at 1 because the setup cost dominates by orders of
# magnitude on real pages; we want call-stack coverage of the
# load, not statistical convergence of the cheap per-render work.
#
# Requires sudo on macOS (dtrace). Requires `cargo install flamegraph`.
#
#   just flame-live https://google.com
flame-live url:
    sudo cargo flamegraph --release --features bench --bin koala \
        -- --bench "{{url}}" --bench-iterations 1 --bench-warmup 0 \
           --width 2048 --height 1536 > /dev/null
    echo "Flamegraph written to flamegraph.svg"

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
