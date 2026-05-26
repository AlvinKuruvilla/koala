---
title: koala WPT conformance
---

# koala WPT conformance

```js
const data = FileAttachment("data/runs.json").json();
```

```js
const runs = (await data).runs;
const latest = runs.at(-1);
```

```js
if (!latest) {
  display(html`<p><em>No runs recorded yet. Capture one with
    <code>just wpt-record /css/CSS2/visudet/</code>, then rebuild the
    dashboard.</em></p>`);
}
```

```js
function fmtPct(x) {
  return x == null ? "—" : (x * 100).toFixed(1) + "%";
}
function fmtDate(iso) {
  if (!iso) return "—";
  return new Date(iso).toLocaleString();
}
```

## Latest run

```js
function crashLink(label) {
  if (!latest) return label;
  const params = new URLSearchParams({
    status: "CRASH",
    run: latest.id,
    area: "(all)",
  });
  return html`<a href="/tests?${params}" style="text-decoration: none; color: inherit;">${label}</a>`;
}
```

<div class="grid grid-cols-4" style="grid-auto-rows: 88px;">
  <div class="card">
    <h2>Pass rate</h2>
    <span class="big">${latest ? fmtPct(latest.pass_rate) : "—"}</span>
  </div>
  <div class="card">
    <h2>Tests run</h2>
    <span class="big">${latest ? latest.total.toLocaleString() : "—"}</span>
  </div>
  <div class="card">
    <h2>Crashes</h2>
    <span class="big">${latest ? crashLink((latest.totals.CRASH ?? 0).toLocaleString()) : "—"}</span>
  </div>
  <div class="card">
    <h2>Recorded</h2>
    <span style="font-size: 0.9rem;">${latest ? fmtDate(latest.timestamp) : "—"}</span>
  </div>
</div>

```js
const totals = latest ? Object.entries(latest.totals).map(([status, count]) => ({status, count})) : [];
```

```js
display(
  Plot.plot({
    title: "Status breakdown — latest run",
    marginLeft: 80,
    x: { label: "Tests", grid: true },
    y: { label: null },
    color: {
      domain: ["PASS", "OK", "FAIL", "CRASH", "TIMEOUT", "ERROR"],
      range: ["#22c55e", "#22c55e", "#f97316", "#ef4444", "#a855f7", "#64748b"],
      legend: false,
    },
    marks: [
      Plot.barX(totals, {
        x: "count",
        y: "status",
        fill: "status",
        sort: { y: "x", reverse: true },
      }),
      Plot.text(totals, {
        x: "count",
        y: "status",
        text: d => d.count.toLocaleString(),
        dx: 8,
        textAnchor: "start",
      }),
    ],
  })
);
```

## Run history

```js
const history = runs.map(r => ({
  id: r.id,
  timestamp: r.timestamp ? new Date(r.timestamp) : null,
  pass_rate: r.pass_rate,
  total: r.total,
  pass: r.totals.PASS ?? 0,
  fail: r.totals.FAIL ?? 0,
  crash: r.totals.CRASH ?? 0,
}));
```

```js
if (history.length < 2) {
  display(html`<p><em>Pass-rate history needs at least two runs. Record
    another with <code>just wpt-record &lt;area&gt;</code>.</em></p>`);
} else {
  display(
    Plot.plot({
      title: "Pass rate over time",
      y: { label: "Pass rate", domain: [0, 1], tickFormat: d => (d * 100).toFixed(0) + "%", grid: true },
      x: { label: "Recorded" },
      marks: [
        Plot.lineY(history, { x: "timestamp", y: "pass_rate", stroke: "#22c55e", strokeWidth: 2 }),
        Plot.dot(history, { x: "timestamp", y: "pass_rate", fill: "#22c55e" }),
      ],
    })
  );
}
```

## All recorded runs

```js
display(
  Inputs.table(
    [...runs].reverse(),
    {
      columns: ["timestamp", "revision", "total", "pass_rate", "id"],
      header: {
        timestamp: "When",
        revision: "Commit",
        total: "Tests",
        pass_rate: "Pass rate",
        id: "Run ID",
      },
      format: {
        timestamp: t => t ? new Date(t).toLocaleString() : "—",
        revision: r => r ? r.slice(0, 8) : "—",
        total: t => t?.toLocaleString() ?? "—",
        pass_rate: fmtPct,
      },
      rows: 20,
    }
  )
);
```
