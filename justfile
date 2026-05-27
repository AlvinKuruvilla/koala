# Open the browser GUI, optionally loading a URL or file path.
#   just gui
#   just gui https://example.com
#   just gui res/test.html
gui url="":
    @if [ -z "{{url}}" ]; then \
        cargo run --bin koala; \
    else \
        cargo run --bin koala -- "{{url}}"; \
    fi

# Run the headless CLI, optionally saving a screenshot.
#   just cli https://example.com
#   just cli res/test.html
#   just cli https://example.com screenshot.png
cli url screenshot="":
    @if [ -z "{{screenshot}}" ]; then \
        cargo run --bin koala-cli -- "{{url}}"; \
    else \
        cargo run --bin koala-cli -- -S "{{screenshot}}" "{{url}}"; \
    fi

# One-time setup for the WPT integration: creates `.venv-wpt/`,
# installs the koala wptrunner plugin, and pulls in wpt's Python
# requirements. Safe to re-run; pip will no-op when versions match.
wpt-setup:
    python3 -m venv .venv-wpt
    .venv-wpt/bin/pip install --upgrade pip
    .venv-wpt/bin/pip install -e wpt-tools/wptrunner-koala
    .venv-wpt/bin/pip install -r third-party/wpt/tools/wptrunner/requirements.txt

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
#   just wpt                                                # smoke test
#   just wpt /css/CSS2/visudet/content-height-001.html      # single test
#   just wpt /dom/nodes/                                    # whole dir
wpt test="/css/CSS2/visudet/content-height-001.html":
    cargo build --release -p koala-cli
    # PYTHONWARNINGS silences wpt-pinned urllib3 v2's
    # `NotOpenSSLWarning` (Python 3.9 on macOS links against
    # LibreSSL, not OpenSSL). The warning is informational and
    # not actionable from our side — wpt's requirements.txt pins
    # urllib3 to exactly 2.6.3.
    PYTHONWARNINGS="ignore::urllib3.exceptions.NotOpenSSLWarning" \
    .venv-wpt/bin/python third-party/wpt/wpt \
        --venv .venv-wpt --skip-venv-setup \
        run \
            --binary="{{justfile_directory()}}/target/release/koala" \
            --no-pause \
            --no-restart-on-unexpected \
            --log-mach=- --log-mach-level=info \
            --log-wptreport=/tmp/koala-wpt.json \
            koala "{{test}}"
    @echo
    @.venv-wpt/bin/python -m wptrunner_koala.summary /tmp/koala-wpt.json

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
