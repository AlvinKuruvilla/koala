---
title: koala WPT
toc: false
---

```js
const db = await DuckDBClient.of({
  results: FileAttachment("data/results.parquet"),
});
const runsData = (await FileAttachment("data/runs.json").json()).runs;
const latest = runsData.at(-1);
const allStatuses = Array.from(
  await db.query("SELECT DISTINCT status FROM results WHERE run_id != '' ORDER BY status"),
  r => r.status
);
const allAreas = Array.from(
  await db.query("SELECT DISTINCT area FROM results WHERE run_id != '' ORDER BY area"),
  r => r.area
);
const allRuns = Array.from(
  await db.query("SELECT DISTINCT run_id FROM results WHERE run_id != '' ORDER BY run_id DESC"),
  r => r.run_id
);
```

```js
// Cross-section reactive state. Each Mutable drives the SQL the test
// grid runs, so clicking a crash reason or an area row pre-filters
// the grid below without us re-architecting around URL params.
const selectedRun = Mutable(allRuns[0] ?? null);
const selectedReason = Mutable(null);
const selectedAreaPick = Mutable(null);
const selectedTest = Mutable(null);
const statusSet = Mutable(new Set(["FAIL", "CRASH", "ERROR", "TIMEOUT"]));
const searchText = Mutable("");

function toggleStatus(s) {
  const next = new Set(statusSet);
  if (next.has(s)) next.delete(s); else next.add(s);
  statusSet.value = next;
}
function clearFilters() {
  selectedReason.value = null;
  selectedAreaPick.value = null;
  searchText.value = "";
}
```

```js
function fmtPct(x) { return x == null ? "—" : (x * 100).toFixed(1) + "%"; }
function fmtNum(n) { return n?.toLocaleString() ?? "—"; }
function fmtDate(iso) { return iso ? new Date(iso).toLocaleString() : "—"; }
function shortRev(r) { return r ? r.slice(0, 8) : "—"; }
```

<div class="dash-hero">
  <h1>koala WPT</h1>
  <div class="dash-hero-meta">
    ${latest
      ? html`<span>${fmtNum(latest.total)} tests · <strong style="color: var(--dash-pass);">${fmtPct(latest.pass_rate)}</strong> pass</span>
             · <code>${shortRev(latest.revision)}</code>
             · ${fmtDate(latest.timestamp)}`
      : html`<em>no runs archived yet</em>`}
  </div>
</div>

```js
if (!latest) {
  display(html`
    <div class="dash-detail" style="margin: 2rem 0;">
      <p style="margin: 0;">
        No runs in <code>dashboard/runs/</code> yet. Record one with
        <code>just wpt-record /css/CSS2/visudet/</code> and refresh.
      </p>
    </div>
  `);
}
```

<div class="dash-kpis">
  <div class="dash-kpi pass">
    <div class="dash-kpi-label">Pass rate</div>
    <div class="dash-kpi-value">${latest ? fmtPct(latest.pass_rate) : "—"}</div>
    <div class="dash-kpi-sub">${latest ? `${fmtNum(latest.totals.PASS ?? 0)} of ${fmtNum(latest.total)}` : ""}</div>
  </div>
  <div class="dash-kpi">
    <div class="dash-kpi-label">Failures</div>
    <div class="dash-kpi-value">${latest ? fmtNum(latest.totals.FAIL ?? 0) : "—"}</div>
    <div class="dash-kpi-sub">pixel mismatches</div>
  </div>
  <div class="dash-kpi crash">
    <div class="dash-kpi-label">Crashes</div>
    <div class="dash-kpi-value">${latest ? fmtNum(latest.totals.CRASH ?? 0) : "—"}</div>
    <div class="dash-kpi-sub">koala-cli panics</div>
  </div>
  <div class="dash-kpi">
    <div class="dash-kpi-label">Other</div>
    <div class="dash-kpi-value">${latest ? fmtNum((latest.totals.ERROR ?? 0) + (latest.totals.TIMEOUT ?? 0)) : "—"}</div>
    <div class="dash-kpi-sub">errors + timeouts</div>
  </div>
</div>

```js
// Pass-rate history sparkline, only when we have >1 run to plot.
if (runsData.length >= 2) {
  display(Plot.plot({
    height: 90,
    marginTop: 10,
    marginLeft: 40,
    marginBottom: 25,
    y: {domain: [0, 1], tickFormat: d => (d * 100).toFixed(0) + "%", grid: true, ticks: 3},
    x: {label: null, ticks: 4},
    marks: [
      Plot.areaY(runsData, {x: d => new Date(d.timestamp), y: "pass_rate", fillOpacity: 0.1, fill: "var(--dash-pass)"}),
      Plot.lineY(runsData, {x: d => new Date(d.timestamp), y: "pass_rate", stroke: "var(--dash-pass)", strokeWidth: 1.5}),
      Plot.dot(runsData, {x: d => new Date(d.timestamp), y: "pass_rate", fill: "var(--dash-pass)", r: 2.5}),
    ],
    style: {fontFamily: "var(--dash-mono)", fontSize: "10px"},
  }));
}
```

<div class="dash-row-2">
  <div>
    <div class="dash-section">Top crash reasons</div>
    <div id="crash-list"></div>
  </div>
  <div>
    <div class="dash-section">Subarea pass rate</div>
    <div id="area-list"></div>
  </div>
</div>

```js
// Top crash reasons in the current run. Click a row to filter the
// test grid to "all crashes with this reason."
const crashRowsTable = await db.query(
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
   GROUP BY 1
   ORDER BY n DESC
   LIMIT 30`,
  [selectedRun]
);
const crashRows = Array.from(crashRowsTable);
const maxCrash = Math.max(...crashRows.map(r => r.n), 1);
```

```js
const sel = selectedReason;
const list = html`<ul class="dash-rank">
  ${crashRows.length === 0
    ? html`<li style="grid-template-columns: 1fr; color: var(--dash-fg-dim); font-style: italic; cursor: default;">No crashes in this run</li>`
    : crashRows.map(r => html`
        <li class=${sel === r.reason ? "selected" : ""}
            onclick=${() => selectedReason.value = sel === r.reason ? null : r.reason}>
          <span class="count">${fmtNum(r.n)}</span>
          <span class="label" title=${r.reason}>${r.reason}</span>
          <span class="bar"><div style=${`width: ${(r.n / maxCrash * 100).toFixed(1)}%`}></div></span>
        </li>
      `)}
</ul>`;
document.querySelector("#crash-list").replaceChildren(list);
```

```js
// Subarea pass rate, sorted ascending — worst-performing first so
// the priority is at the top. Click to filter the test grid to that
// subarea.
const areaRowsTable = await db.query(
  `SELECT area,
          COUNT(*)::INTEGER AS total,
          SUM(CASE WHEN status IN ('PASS','OK') THEN 1 ELSE 0 END)::INTEGER AS pass,
          (SUM(CASE WHEN status IN ('PASS','OK') THEN 1 ELSE 0 END)::DOUBLE / NULLIF(COUNT(*),0)) AS pass_rate
   FROM results
   WHERE run_id = ?
   GROUP BY 1
   ORDER BY pass_rate ASC, total DESC`,
  [selectedRun]
);
const areaRows = Array.from(areaRowsTable);
```

```js
const sel = selectedAreaPick;
const list = html`<ul class="dash-rank">
  ${areaRows.length === 0
    ? html`<li style="grid-template-columns: 1fr; color: var(--dash-fg-dim); font-style: italic; cursor: default;">No areas in this run</li>`
    : areaRows.map(r => html`
        <li class=${sel === r.area ? "selected" : ""}
            onclick=${() => selectedAreaPick.value = sel === r.area ? null : r.area}>
          <span class="count" style=${`color: ${r.pass_rate > 0.5 ? "var(--dash-pass)" : "var(--dash-fail)"}`}>${(r.pass_rate * 100).toFixed(0)}%</span>
          <span class="label" title=${`${r.area} — ${r.pass}/${r.total}`}>${r.area}</span>
          <span class="bar"><div style=${`width: ${(r.pass_rate * 100).toFixed(1)}%; background: ${r.pass_rate > 0.5 ? "var(--dash-pass)" : "var(--dash-fail)"}`}></div></span>
        </li>
      `)}
</ul>`;
document.querySelector("#area-list").replaceChildren(list);
```

<div class="dash-section">Tests</div>

```js
// Active-filter banner — only when at least one cross-section pick
// is set. Clicking "Clear" resets both reason and area selections.
const _reason = selectedReason;
const _area = selectedAreaPick;
if (_reason || _area) {
  const parts = [];
  if (_area) parts.push(html`area <code>${_area}</code>`);
  if (_reason) parts.push(html`reason <code>${_reason}</code>`);
  display(html`<div class="dash-active-filter" style="margin-bottom: 0.5rem;">
    Filtering by ${parts.flatMap((p, i) => i === 0 ? [p] : [" and ", p])}
    <button onclick=${() => clearFilters()}>Clear</button>
  </div>`);
}
```

```js
// Status chips + search + run picker.
const _statusSet = statusSet;
const _searchText = searchText;
const _selectedRun = selectedRun;

const chipRow = html`<div class="dash-chips">
  ${allStatuses.map(s => html`<span
    class=${`dash-chip ${_statusSet.has(s) ? "active " + s.toLowerCase() : ""}`}
    onclick=${() => toggleStatus(s)}>${s}</span>`)}
</div>`;

const searchInput = Object.assign(
  html`<input type="search" placeholder="search test path (e.g. flexbox, abrupt-doctype, grid-template)…" value=${_searchText}/>`,
  {oninput(e) { searchText.value = e.target.value; }}
);

const runSelect = (() => {
  const sel = html`<select></select>`;
  for (const r of allRuns) {
    const opt = html`<option value=${r}>${r}</option>`;
    if (r === _selectedRun) opt.selected = true;
    sel.appendChild(opt);
  }
  sel.onchange = (e) => { selectedRun.value = e.target.value; };
  return sel;
})();

display(html`<div class="dash-filterbar">
  ${chipRow}
  ${searchInput}
  ${allRuns.length > 1 ? runSelect : ""}
  <span class="dash-filterbar-count" id="match-count">—</span>
</div>`);
```

```js
// Build and execute the filtered query. We interpolate enumerated
// values (status, area) into the SQL since they come from <select>s
// over the data's own distinct values; the user-supplied substring
// and the reason filter are passed as bound parameters.
const statusList = [...statusSet].length === 0
  ? "''"
  : [...statusSet].map(s => `'${s.replace(/'/g, "''")}'`).join(",");
const areaPred = selectedAreaPick
  ? `area = '${selectedAreaPick.replace(/'/g, "''")}'`
  : "1=1";
const reasonPred = selectedReason ? "message ILIKE '%' || ? || '%'" : "1=1";

const queryParams = [selectedRun, searchText, searchText];
if (selectedReason) queryParams.push(selectedReason);

const filteredTable = await db.query(
  `SELECT test, status, area, duration_ms, message
   FROM results
   WHERE run_id = ?
     AND status IN (${statusList})
     AND ${areaPred}
     AND (? = '' OR test ILIKE '%' || ? || '%')
     AND ${reasonPred}
   ORDER BY
     CASE status WHEN 'CRASH' THEN 0 WHEN 'ERROR' THEN 1 WHEN 'TIMEOUT' THEN 2
                 WHEN 'FAIL' THEN 3 WHEN 'PASS' THEN 4 ELSE 5 END,
     test
   LIMIT 2000`,
  queryParams
);
const filtered = Array.from(filteredTable, r => ({
  test: r.test,
  status: r.status,
  area: r.area,
  duration_ms: r.duration_ms,
  message: r.message,
}));

const totalMatchTable = await db.query(
  `SELECT COUNT(*)::INTEGER AS n
   FROM results
   WHERE run_id = ?
     AND status IN (${statusList})
     AND ${areaPred}
     AND (? = '' OR test ILIKE '%' || ? || '%')
     AND ${reasonPred}`,
  queryParams
);
const totalMatch = Array.from(totalMatchTable)[0]?.n ?? 0;

// Update the count in the filter bar (sibling element, easier than
// threading through reactive cells).
const countEl = document.querySelector("#match-count");
if (countEl) {
  countEl.textContent =
    totalMatch > 2000
      ? `${totalMatch.toLocaleString()} matches · showing first 2,000`
      : `${totalMatch.toLocaleString()} matches`;
}
```

<div class="dash-row-split">
  <div id="tests-list"></div>
  <div id="tests-detail"></div>
</div>

```js
const _selected = selectedTest;
const rows = filtered;
const table = html`<div class="dash-tests"><table>
  <thead><tr>
    <th>Test</th><th>Status</th><th>Duration</th>
  </tr></thead>
  <tbody>
    ${rows.length === 0
      ? html`<tr><td colspan="3" style="padding: 1rem; text-align: center; color: var(--dash-fg-dim); font-style: italic;">No tests match the current filter.</td></tr>`
      : rows.map(r => html`<tr
          class=${_selected === r.test ? "selected" : ""}
          onclick=${() => selectedTest.value = r.test}>
          <td title=${r.test}>${r.test}</td>
          <td><span class=${`dash-status ${r.status}`}>${r.status}</span></td>
          <td style="text-align: right; color: var(--dash-fg-muted);">${r.duration_ms ?? "—"}</td>
        </tr>`)}
  </tbody>
</table></div>`;
document.querySelector("#tests-list").replaceChildren(table);
```

```js
function preBlock(text) {
  return html`<pre>${text}</pre>`;
}

function formatMessage(status, raw) {
  if (!raw) {
    return status === "PASS"
      ? html`<p class="empty">Test passed.</p>`
      : html`<p class="empty">No message recorded for this ${status}.</p>`;
  }

  if (status === "CRASH") {
    const m = raw.match(/panicked at ([^\n]+):\s*\n([^\n]+)/);
    if (m) {
      return html`
        <div class="label">Panic at</div>
        <div><code>${m[1]}</code></div>
        <div class="label">Reason</div>
        <div><code>${m[2]}</code></div>
        <details style="margin-top: 0.5rem;">
          <summary>Full panic output</summary>
          ${preBlock(raw)}
        </details>
      `;
    }
  }

  if (status === "ERROR") {
    const m = raw.match(/^koala load_failed for (\S+):\s*(.+)/s);
    if (m) {
      return html`
        <div class="label">Failed to load</div>
        <div><code style="word-break: break-all;">${m[1]}</code></div>
        <div class="label">Reason</div>
        <div>${m[2].split("\n")[0].trim()}</div>
        <details style="margin-top: 0.5rem;">
          <summary>Full error</summary>
          ${preBlock(raw)}
        </details>
      `;
    }
  }

  if (status === "FAIL") {
    const lines = raw.trim().split("\n").filter(Boolean);
    if (lines.length === 2) {
      const t = lines[0].match(/^(\/\S+)\s+\['([0-9a-f]+)'\]/);
      const r = lines[1].match(/^(\/\S+)\s+\['([0-9a-f]+)'\]/);
      if (t && r) {
        return html`
          <p style="margin: 0 0 0.5rem;">
            <strong>Pixel mismatch.</strong> koala's render differs
            from the reference. Re-run with
            <code>--reftest-screenshot=always</code> to archive both
            PNGs.
          </p>
          <div class="label">Test</div>
          <div><code>${t[1]}</code> <span style="color: var(--dash-fg-dim);">(${t[2].slice(0, 12)})</span></div>
          <div class="label">Ref</div>
          <div><code>${r[1]}</code> <span style="color: var(--dash-fg-dim);">(${r[2].slice(0, 12)})</span></div>
        `;
      }
    }
  }

  return preBlock(raw);
}
```

```js
const _selectedTest = selectedTest;
const row = filtered.find(r => r.test === _selectedTest);
const panel = !_selectedTest
  ? html`<div class="dash-detail"><p class="empty">Select a test on the left to see its failure detail.</p></div>`
  : !row
  ? html`<div class="dash-detail"><p class="empty">The selected test isn't in the current filter. Adjust filters or click another row.</p></div>`
  : html`<div class="dash-detail">
      <h3>${row.test}</h3>
      <div class="meta">
        <span class=${`dash-status ${row.status}`}>${row.status}</span>
        · <code>${row.area}</code>
        · ${row.duration_ms ?? "—"} ms
      </div>
      ${formatMessage(row.status, row.message)}
    </div>`;
document.querySelector("#tests-detail").replaceChildren(panel);
```
