---
title: By area
---

# Pass rate by WPT area

```js
const data = FileAttachment("data/runs.json").json();
```

```js
const runs = (await data).runs;
const latest = runs.at(-1);
```

```js
if (!latest) {
  display(html`<p><em>No runs recorded yet — start with
    <code>just wpt-record /css/CSS2/visudet/</code>.</em></p>`);
}
```

```js
const areas = latest?.areas ?? [];
```

```js
function fmtPct(x) {
  return x == null ? "—" : (x * 100).toFixed(1) + "%";
}
```

## Latest run (${latest?.id ?? "—"})

```js
if (areas.length === 0) {
  display(html`<p><em>This run has no test results.</em></p>`);
} else {
  display(
    Plot.plot({
      title: "Pass rate by area",
      marginLeft: 120,
      x: { domain: [0, 1], tickFormat: d => (d * 100).toFixed(0) + "%", grid: true, label: "Pass rate" },
      y: { label: null },
      color: { type: "linear", range: ["#ef4444", "#22c55e"], domain: [0, 1] },
      marks: [
        Plot.barX(areas, {
          x: "pass_rate",
          y: "area",
          fill: "pass_rate",
          sort: { y: "x", reverse: true },
        }),
        Plot.text(areas, {
          x: "pass_rate",
          y: "area",
          text: d => `${d.PASS}/${d.total}`,
          dx: 6,
          textAnchor: "start",
          fontSize: 11,
        }),
      ],
      height: Math.max(160, areas.length * 24 + 60),
    })
  );
}
```

## Detail table

Click any **failures** count to drill in to that subarea's
non-passing tests on the [Tests](/tests) page.

```js
function testsLink(area, statuses, label) {
  const params = new URLSearchParams({
    area,
    status: statuses.join(","),
    run: latest.id,
  });
  return html`<a href="/tests?${params}">${label}</a>`;
}
```

```js
display(
  Inputs.table(
    areas,
    {
      columns: ["area", "total", "PASS", "FAIL", "CRASH", "TIMEOUT", "ERROR", "pass_rate"],
      header: {
        area: "Area",
        total: "Tests",
        pass_rate: "Pass rate",
      },
      format: {
        FAIL: (n, i) => n ? testsLink(areas[i].area, ["FAIL"], n.toLocaleString()) : "0",
        CRASH: (n, i) => n ? testsLink(areas[i].area, ["CRASH"], n.toLocaleString()) : "0",
        TIMEOUT: (n, i) => n ? testsLink(areas[i].area, ["TIMEOUT"], n.toLocaleString()) : "0",
        ERROR: (n, i) => n ? testsLink(areas[i].area, ["ERROR"], n.toLocaleString()) : "0",
        pass_rate: fmtPct,
      },
      sort: "pass_rate",
      reverse: true,
      rows: 50,
    }
  )
);
```
