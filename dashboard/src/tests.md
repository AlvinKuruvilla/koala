---
title: Tests
---

# Per-test results

Filter the full per-test grid by status, subarea, run, and substring.
Backed by a Parquet file queried client-side with DuckDB-WASM — every
filter change re-runs a SQL query against the result set with no
network round-trip.

```js
const db = await DuckDBClient.of({
  results: FileAttachment("data/results.parquet")
});
```

```js
// DuckDBClient.query() returns an Apache Arrow Table — iterable but
// not a JS array, so we convert. queryRow() handles single-row reads.
async function rows(sql, params) {
  return Array.from(await db.query(sql, params));
}
```

```js
// Drop the empty-schema placeholder row the loader ships when no
// runs are archived yet.
const totalRowsRow = await db.queryRow(
  "SELECT COUNT(*)::INTEGER AS n FROM results WHERE run_id != ''"
);
const totalRows = totalRowsRow?.n ?? 0;
```

```js
if (totalRows === 0) {
  display(html`<p><em>No runs recorded yet. Capture one with
    <code>just wpt-record /css/CSS2/visudet/</code>, then rebuild the
    dashboard.</em></p>`);
}
```

```js
// Pull the dimension values once at page load — these drive the
// filter Inputs.
const allStatuses = Array.from(
  await db.query("SELECT DISTINCT status FROM results WHERE run_id != '' ORDER BY status"),
  row => row.status
);
const allAreas = Array.from(
  await db.query("SELECT DISTINCT area FROM results WHERE run_id != '' ORDER BY area"),
  row => row.area
);
const allRuns = Array.from(
  await db.query("SELECT DISTINCT run_id FROM results WHERE run_id != '' ORDER BY run_id DESC"),
  row => row.run_id
);
```

```js
// Read pre-applied filter values from the URL so links from /areas
// can deep-link into "show me all CRASHes in css/CSS2".
const params = new URLSearchParams(location.search);
const initialStatuses = params.get("status")?.split(",").filter(Boolean) ?? ["CRASH", "FAIL"];
const initialArea = params.get("area") ?? "(all)";
const initialRun = params.get("run") ?? (allRuns[0] ?? "");
const initialSearch = params.get("q") ?? "";
```

## Filters

```js
const statuses = view(Inputs.checkbox(allStatuses, {
  label: "Status",
  value: initialStatuses.filter(s => allStatuses.includes(s)),
}));
```

```js
const area = view(Inputs.select(["(all)", ...allAreas], {
  label: "Subarea",
  value: allAreas.includes(initialArea) ? initialArea : "(all)",
}));
```

```js
const run = view(Inputs.select(allRuns, {
  label: "Run",
  value: allRuns.includes(initialRun) ? initialRun : (allRuns[0] ?? ""),
}));
```

```js
const search = view(Inputs.text({
  label: "Test path contains",
  value: initialSearch,
  placeholder: "e.g. flexbox, grid-template, abrupt-doctype",
}));
```

```js
// Keep the URL in sync with the current filter selection.
{
  const url = new URL(location.href);
  url.searchParams.set("status", statuses.join(","));
  url.searchParams.set("area", area);
  url.searchParams.set("run", run);
  if (search) url.searchParams.set("q", search); else url.searchParams.delete("q");
  history.replaceState({}, "", url);
}
```

## Results

```js
// Status and area come from <select>/<checkbox> inputs whose values
// are enumerated from the data, so interpolating them into SQL is
// safe. The user-supplied substring goes through prepared-statement
// binding (the `?` placeholders).
const statusList = statuses.length === 0
  ? "''"
  : statuses.map(s => `'${s.replace(/'/g, "''")}'`).join(",");
const areaPred = area === "(all)" ? "1=1" : `area = '${area.replace(/'/g, "''")}'`;

const filteredCountRow = await db.queryRow(
  `SELECT COUNT(*)::INTEGER AS n FROM results
     WHERE run_id = ?
       AND status IN (${statusList})
       AND ${areaPred}
       AND (? = '' OR test ILIKE '%' || ? || '%')`,
  [run, search, search]
);
const filteredCount = filteredCountRow?.n ?? 0;

const filtered = await db.query(
  `SELECT test, status, area, duration_ms, message
   FROM results
   WHERE run_id = ?
     AND status IN (${statusList})
     AND ${areaPred}
     AND (? = '' OR test ILIKE '%' || ? || '%')
   ORDER BY status, test
   LIMIT 5000`,
  [run, search, search]
);
```

## Top crash reasons in this filter

```js
// Group CRASH-status rows by parsed panic reason. The `not yet
// implemented:` panics are the dominant koala crash kind today, so
// we extract that line first; otherwise fall back to the file:line
// of the panic site. DuckDB's regexp_extract takes a 1-indexed
// capture group.
const crashReasonsTable = await db.query(
  `SELECT
     COALESCE(
       NULLIF(regexp_extract(message, 'not yet implemented: ([^\n]+)', 1), ''),
       NULLIF(regexp_extract(message, 'panicked at ([^\n]+):', 1), ''),
       '(unparsed)'
     ) AS reason,
     COUNT(*)::INTEGER AS n
   FROM results
   WHERE run_id = ?
     AND status = 'CRASH'
     AND ${areaPred}
   GROUP BY 1
   ORDER BY n DESC
   LIMIT 20`,
  [run]
);
const crashReasons = Array.from(crashReasonsTable, r => ({
  reason: r.reason,
  count: r.n,
}));
```

```js
if (crashReasons.length === 0) {
  display(html`<p><em>No crashes in this filter — every test ran to a real result.</em></p>`);
} else {
  display(
    Inputs.table(crashReasons, {
      columns: ["count", "reason"],
      header: { count: "Crashes", reason: "Reason (parsed from panic)" },
      width: { count: 80, reason: 800 },
      rows: 20,
      layout: "fixed",
    })
  );
}
```

## Results

<p style="margin: 0 0 0.5rem 0;">
  <strong>${filteredCount.toLocaleString()}</strong> matching rows
  ${filteredCount > 5000 ? html`(showing first 5,000)` : ""}
</p>

```js
display(
  Inputs.table(filtered, {
    columns: ["test", "status", "area", "duration_ms"],
    header: {
      test: "Test",
      status: "Status",
      area: "Subarea",
      duration_ms: "Duration (ms)",
    },
    width: { test: 480, status: 80, area: 140, duration_ms: 100 },
    rows: 30,
    layout: "fixed",
  })
);
```

## Selected test detail

```js
// Inputs.table can act as a form input; when used inside `view()` it
// returns the selected row(s). `multiple: false` returns a single
// row (or `null` when nothing is selected).
const selected = view(
  Inputs.table(filtered, {
    columns: ["test", "status"],
    header: { test: "Click a row to see the failure message", status: "Status" },
    rows: 8,
    multiple: false,
    width: { test: 480, status: 80 },
    layout: "fixed",
  })
);
```

```js
function preBlock(text) {
  return html`<pre style="white-space: pre-wrap; word-break: break-word;
       background: var(--theme-background-alt); padding: 0.75rem;
       border-radius: 4px; font-size: 0.85em;">${text}</pre>`;
}

// Parse the message field into a structured shape so the panel can
// surface the actionable bit (panic reason, network error, pixel
// mismatch) prominently and tuck the full text under a <details>.
function formatMessage(status, raw) {
  if (!raw) {
    return status === "PASS"
      ? html`<p style="opacity: 0.7;"><em>Test passed.</em></p>`
      : html`<p style="opacity: 0.7;"><em>No message recorded for this ${status}.</em></p>`;
  }

  // CRASH — koala-cli panicked. Pull out file:line and the
  // not-yet-implemented reason; both are emitted by Rust's default
  // panic handler in the format
  //   panicked at <file:line:col>:
  //   <reason>
  if (status === "CRASH") {
    const m = raw.match(/panicked at ([^\n]+):\s*\n([^\n]+)/);
    if (m) {
      const [, location, reason] = m;
      return html`
        <p><strong>Panic at:</strong> <code>${location}</code></p>
        <p><strong>Reason:</strong> <code style="background: var(--theme-background-alt); padding: 2px 6px; border-radius: 3px;">${reason}</code></p>
        <details>
          <summary style="cursor: pointer; opacity: 0.7;">Full crash output</summary>
          ${preBlock(raw)}
        </details>
      `;
    }
  }

  // ERROR — koala-cli emitted a `load_failed` event, the executor
  // surfaces it as "koala load_failed for <url>: <reason>".
  if (status === "ERROR") {
    const m = raw.match(/^koala load_failed for (\S+):\s*(.+)/s);
    if (m) {
      const [, url, rest] = m;
      const reason = rest.split("\n")[0].trim();
      return html`
        <p><strong>Failed to load:</strong> <code style="word-break: break-all;">${url}</code></p>
        <p><strong>Reason:</strong> ${reason}</p>
        <details>
          <summary style="cursor: pointer; opacity: 0.7;">Full error output</summary>
          ${preBlock(raw)}
        </details>
      `;
    }
  }

  // FAIL — for reftests, wptrunner's "message" is just the test
  // path + screenshot hash followed by the ref path + ref hash,
  // separated by a newline. There's no semantic diff: it's "the
  // pixels didn't match." Surface that explicitly so the user
  // isn't squinting at hex hashes wondering what failed.
  if (status === "FAIL") {
    const lines = raw.trim().split("\n").filter(Boolean);
    if (lines.length === 2) {
      const t = lines[0].match(/^(\/\S+)\s+\['([0-9a-f]+)'\]/);
      const r = lines[1].match(/^(\/\S+)\s+\['([0-9a-f]+)'\]/);
      if (t && r) {
        return html`
          <p><strong>Pixel mismatch</strong> — koala's rendering of the test
            differs from the reference. wptrunner doesn't archive screenshots
            by default; re-run with <code>--reftest-screenshot=always</code>
            to capture both PNGs.</p>
          <p><strong>Test:</strong> <code>${t[1]}</code><br>
             <span style="opacity: 0.7;">hash <code>${t[2].slice(0, 12)}</code></span></p>
          <p><strong>Ref:</strong> <code>${r[1]}</code><br>
             <span style="opacity: 0.7;">hash <code>${r[2].slice(0, 12)}</code></span></p>
        `;
      }
    }
  }

  return preBlock(raw);
}
```

```js
if (!selected) {
  display(html`<p><em>Select a row above to see its failure
    message and surrounding context.</em></p>`);
} else {
  // `selected` is an Arrow RowProxy; pull the columns we want into a
  // plain object so template literals and the message handlers work
  // without RowProxy surprises.
  const row = {
    test: selected.test,
    status: selected.status,
    area: selected.area,
    duration_ms: selected.duration_ms,
    message: selected.message,
  };
  display(html`
    <div class="card">
      <h3 style="margin-top: 0; font-family: var(--monospace); word-break: break-all;">${row.test}</h3>
      <p>
        <strong>Status:</strong> ${row.status} ·
        <strong>Area:</strong> ${row.area} ·
        <strong>Duration:</strong> ${row.duration_ms ?? "—"} ms
      </p>
      ${formatMessage(row.status, row.message)}
    </div>
  `);
}
```
