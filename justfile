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
#   just wpt                                                # smoke test
#   just wpt /css/CSS2/visudet/content-height-001.html      # single test
#   just wpt /css/CSS2/visudet/                             # whole dir
wpt test="/css/CSS2/visudet/content-height-001.html":
    cargo build --release -p koala-cli
    .venv-wpt/bin/python third-party/wpt/wpt \
        --venv .venv-wpt --skip-venv-setup \
        run \
            --binary="{{justfile_directory()}}/target/release/koala" \
            --no-pause \
            --no-restart-on-unexpected \
            --log-mach=- --log-mach-level=info \
            koala "{{test}}"

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

# Install the dashboard's Node dependencies (Observable Framework).
# One-time per checkout; safe to re-run.
dashboard-setup:
    cd dashboard && npm install

# Build the static dashboard into dashboard/dist/. The Observable
# data loader re-runs on every build, so the dashboard always
# reflects whatever is currently in dashboard/runs/.
dashboard-build:
    cd dashboard && npm run build

# Start the Observable preview server on http://127.0.0.1:3000 with
# hot reload. Edit src/*.md and the page re-renders automatically.
dashboard-serve:
    cd dashboard && npm run dev

# Tear down the wpt venv and clean up any koala temp screenshots
# left behind by interrupted runs.
wpt-clean:
    rm -rf .venv-wpt
    find /tmp /var/folders -name 'koala-wpt-*.png' -delete 2>/dev/null || true
