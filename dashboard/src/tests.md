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
// Drop the empty-schema placeholder row that the data loader ships
// when no runs are archived yet.
const totalRows = (await db.query(
  "SELECT COUNT(*)::INTEGER AS n FROM results WHERE run_id != ''"
))[0].n;
```

```js
if (totalRows === 0) {
  display(html`<p><em>No runs recorded yet. Capture one with
    <code>just wpt-record /css/CSS2/visudet/</code>, then rebuild the
    dashboard.</em></p>`);
}
```

```js
// Pull the dimension values once at page load — these are tiny and
// drive the filter Inputs.
const allStatuses = (await db.query(
  "SELECT DISTINCT status FROM results WHERE run_id != '' ORDER BY status"
)).map(r => r.status);
const allAreas = (await db.query(
  "SELECT DISTINCT area FROM results WHERE run_id != '' ORDER BY area"
)).map(r => r.area);
const allRuns = (await db.query(
  "SELECT DISTINCT run_id FROM results WHERE run_id != '' ORDER BY run_id DESC"
)).map(r => r.run_id);
```

```js
// Read pre-applied filter values from the URL so links from
// /areas can deep-link into "show me all CRASHes in css/CSS2".
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
// Keep the URL in sync with the current filter selection so the user
// can bookmark or share.
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
// Build the SQL predicate from the live filter values. DuckDB-WASM
// supports parameter binding; we use it for the substring search to
// avoid quote-escaping. `area` and `status` come from select inputs
// so their values are already enumerated and safe to interpolate.
const statusList = statuses.length === 0
  ? "''"
  : statuses.map(s => `'${s.replace(/'/g, "''")}'`).join(",");
const areaPred = area === "(all)" ? "1=1" : `area = '${area.replace(/'/g, "''")}'`;
const sql = `
  SELECT test, status, area, duration_ms, message
  FROM results
  WHERE run_id = ?
    AND status IN (${statusList})
    AND ${areaPred}
    AND (? = '' OR test ILIKE '%' || ? || '%')
  ORDER BY status, test
  LIMIT 5000
`;
const filtered = await db.query(sql, [run, search, search]);
```

```js
const filteredCount = (await db.query(
  `SELECT COUNT(*)::INTEGER AS n FROM results
     WHERE run_id = ?
       AND status IN (${statusList})
       AND ${areaPred}
       AND (? = '' OR test ILIKE '%' || ? || '%')`,
  [run, search, search]
))[0].n;
```

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
// `Inputs.table` returns the selected rows when used as a view.
// Default `multiple: true` returns an array; with `required: false`
// the empty selection is `[]`.
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
if (!selected || (Array.isArray(selected) && selected.length === 0)) {
  display(html`<p><em>Select a row above to see its failure
    message and surrounding context.</em></p>`);
} else {
  const row = Array.isArray(selected) ? selected[0] : selected;
  display(html`
    <div class="card">
      <h3 style="margin-top: 0; font-family: var(--monospace);">${row.test}</h3>
      <p>
        <strong>Status:</strong> ${row.status} ·
        <strong>Area:</strong> ${row.area} ·
        <strong>Duration:</strong> ${row.duration_ms ?? "—"} ms
      </p>
      ${row.message
        ? html`<pre style="white-space: pre-wrap; background: var(--theme-background-alt); padding: 0.75rem; border-radius: 4px;">${row.message}</pre>`
        : html`<p><em>(no message — the test ${row.status === "PASS" ? "passed" : "produced no diagnostic"})</em></p>`}
    </div>
  `);
}
```
