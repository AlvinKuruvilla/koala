# What Koala can offer AI agents that Chrome cannot

Research of 2026-10-15/16 into where a from-scratch engine gives agents
something genuinely useful that Chrome, Chrome DevTools MCP, and the
agent-specific browsers (Lightpanda, Obscura, hosted Chromium fleets) do
not. The premise, set by the owner: agents drive Koala through the
existing `chrome-devtools-mcp` over CDP, so behaviour matches Chrome;
anything Koala adds must be a concrete win on top of that.

Four research passes fed this: agent developers' measured pain points,
the product landscape, Chrome's architectural limits, and an inventory
of Koala itself. Almost every performance number below is
vendor-reported; the exceptions are marked. Sources are at the end.

## Findings

### What hurts agents, by evidence

1. **Page representation size and quality** is the best-measured
   problem. One accessibility snapshot of a Wikipedia page measured
   293k tokens; Playwright MCP tasks average about 114k tokens against
   about 27k for its CLI, and Microsoft's own README now steers coding
   agents to the CLI. ComponentBench (Aug 2026) found that changing only
   the observation and action space moves success by more than 30
   points for the same model. The most-reacted playwright-mcp issues ask
   for smaller snapshots (#395, #889).
2. **Prompt injection from page content.** Anthropic measured 23.6%
   attack success before mitigations and 11.2% after (123 cases,
   Aug 2025); its browser-specific set (hidden DOM fields, URL text, tab
   titles) went from 35.7% to 0%. TRAP (Dec 2025) found agents
   susceptible in 25% of tasks. The published incidents against Comet
   and Opera Neon used text the user could not see: spoiler-hidden
   comments, non-rendered HTML, near-background text read by OCR.
   Counter-evidence: CUA-HandCrafted (Jun 2026) found 0 of 140
   hand-crafted attacks succeeding against current models.
3. **Knowing when a page is ready.** CDP signals load events, not "the
   page has settled". Timeout issues are common in every agent-browser
   repo; no study measures what share of failures they cause.
4. **Bot detection.** Real, but a new engine makes it worse: an unknown
   fingerprint fails checks Chromium passes. An independent survey
   (Apr 2026) found all eleven hosted stealth services detectable.
5. **Most task failures are the agent's own** (planning, filtering,
   loops). No engine fixes those.

Thin evidence: irreversible actions and undo (one 18-reaction issue,
ST-WebAgentBench), and reproducing failed runs (no measurement found).

### What Chrome can and cannot do

Chrome can do more than assumed. `DOMSnapshot.captureSnapshot` returns
paint order, rects, blended background colours and text opacity;
`Emulation.setVirtualTimePolicy` and `HeadlessExperimental.beginFrame`
still exist (experimental, headless-shell only). Agent tools do not use
these: Chrome DevTools MCP's `take_snapshot` and Playwright's
`browser_snapshot` serialize the accessibility tree, which deliberately
keeps visually hidden text because screen-reader-only content depends
on it.

What is architecturally hard for Chrome:

- **Forking a live page.** State spans the browser, renderer, GPU and
  network processes; agent infrastructure snapshots whole VMs instead
  (E2B about 4 s per GiB; Firecracker forks about 150 ms).
- **Per-action change reports.** DOM events cover only nodes the client
  already holds; repaint information stays in the compositor.
- **An exact, single-frame visibility verdict.** The primitives exist
  but come from separate snapshots across processes.

### Who else is in this space

- **Lightpanda** (Zig + V8, AGPL-3.0 with a CLA, 1.0.0 on 2026-10-02)
  has no layout and no paint. Screenshots are reader-mode text;
  `getBoundingClientRect` is a "faux-layout"; its visibility check stops
  at `display`, `visibility` and `opacity`. Its tracker shows demand for
  real geometry (#3921: a flex child reports 5 px where Chrome gives
  760 px), screenshots (#507, #2228) and screencasts (#2140). A maintainer
  called visibility "very useful for agentic and testing use cases" and
  said nobody is working on rendering. It already links a Rust staticlib
  (`src/rust/`), so a Rust layout crate has a place to plug in, but no
  DOM handoff from Zig to Rust exists yet.
- **Obscura** (Rust + V8, Apache-2.0) claims layout, paint, screenshots
  and drop-in Puppeteer/Playwright support. Unverified; it is the
  closest overlap with Koala.
- **Blitz** (Rust: Taffy layout, Parley text, no JS) and **Servo**
  (embeddable since 0.1.0, April 2026) answer "why not use an existing
  Rust engine?".
- **Hosted Chromium fleets** (Browserbase, Steel, Kernel, Browser Use,
  Bright Data) sell operations: sessions, recording, stealth, CAPTCHA
  solving. None changes what the engine tells the agent.

No product found labels page content by whether a person can actually
see it, using layout, clipping, paint order and contrast (medium
confidence).

### Where Koala stands

- Layout, line fragments and the display list exist as plain Rust data;
  hit testing and `:hover` work.
- **JavaScript runs only during load**: Boa's context is dropped
  afterwards (`koala-ui/src/browser_page.rs:33`), so clicks and input
  cannot reach page scripts. Every interactive feature waits on this.
- No CDP server, no accessibility tree, no forms or text input, no
  scrolling. Link clicks exist on the unmerged `ui/pointer` branch.
- Web compatibility: 29% of 24,095 WPT tests passed on 2026-05-26.
- Boa cannot clone its heap or seed `Math.random`.

## Recommendation

### The differentiator: a snapshot of what a person sees

Make Koala's answer to the accessibility snapshot carry visibility from
the display list: for each node, whether it is drawn, unclipped,
on-screen, unoccluded and of readable contrast and size, and why not
when it is not. Hidden content is dropped or labelled.

- It addresses the two best-evidenced pain points at once: it removes
  what a person cannot see (prompt injection) and shrinks the snapshot
  (token cost).
- It needs exactly what Koala has and Lightpanda lacks: layout, clips,
  paint order and colours, in one process and one frame.
- It reaches agents through the tool they already use.
  `chrome-devtools-mcp`'s `take_snapshot` reads the CDP accessibility
  tree, so Koala's implementation of the `Accessibility` domain can
  return the filtered tree; the agent changes nothing. The full tree
  stays available on request for screen-reader-only content.

### Two protocol-level improvements

These follow from owning the pipeline, and are design constraints to
honour as worker threads and GPU work arrive, not properties Koala has
by being single-threaded:

- **A settled signal**: every worker reports its outstanding work
  (timers due, fetches in flight, pending restyle, layout or paint), and
  navigation or action completion waits until all of it drains.
- **Change reports**: after an action, the nodes restyled, re-laid-out
  and repainted, from the dirty tracking the invalidation work will add.
  This also lets a client send a diff instead of a fresh snapshot.

### Not worth pursuing now

- **Throughput and memory**: Lightpanda owns this, and it only counts on
  pages rendered correctly.
- **Stealth**: a new engine is easier to detect, not harder.
- **Forking and rollback**: Chrome cannot do it and Koala could for the
  DOM and layout, but Boa cannot clone its heap and demand evidence is
  thin.
- **Determinism for agent evaluations**: demand is unproven, and the
  measured run-to-run variance is mostly the model's. Keep a software
  raster path and controllable clock as a constraint.
- **Explanations of layout and cascade**: rejected by the owner, as no
  appetite is expected.

### Building on Lightpanda

Possible and complementary: Koala's layout and visibility as a Rust crate
behind Lightpanda's FFI would give it real boxes, honest Playwright
actionability checks, visibility and screenshots. Costs: the combined
binary is AGPL; the Zig-to-Rust DOM and computed-style handoff has to be
built; upstream acceptance is unknown (Blitz's maintainer has already
pitched Taffy and Parley there); and Koala's layout has to be right on
real pages first. Worth keeping the visibility and layout code a
standalone crate either way, and revisiting once it works on Koala's own
corpus.

### Order of work

1. Keep JavaScript alive after load (the Boa context lifetime problem).
   Nothing interactive works without it.
2. A CDP server covering what `chrome-devtools-mcp` needs to attach
   (Target, Page, Runtime, DOM, Input) plus `Accessibility`, and an
   accessibility tree with roles and names.
3. Visibility from the display list feeding that tree.

Before step 3, check the claim with data: on the bench corpus, measure
how often Chrome's accessibility snapshot contains text that is not
visible, and assemble a test set from the published hidden-text attacks.
If real pages rarely carry invisible text outside screen-reader-only
content, the token-size half of the argument has to carry it alone.

## Forking and agent workloads (2026-10-16)

The owner judged visibility filtering too small to justify the project
and asked for two follow-ups: whether Boa can fork a live heap, and
which agent problems a from-scratch engine could reasonably solve.
Web compatibility is treated as work in progress, not a disqualifier;
advantages are split into structural ones and ones that depend on it.

### Boa cannot fork its heap

Read in the Boa 0.22 source:

- Every allocation is its own `Box` linked by raw pointers, tracked in a
  thread-local list (`boa_gc/src/lib.rs:131-146`). `Trace` only visits
  pointers; nothing can copy them. A pointer-remapping clone means a new
  trait and VTable slot across about 250 `Trace` types: a long-lived fork
  of Boa.
- Pending promise jobs are boxed `FnOnce` closures (`job.rs:61-64`) and
  cannot be copied, so a clone is only possible with an empty microtask
  queue (Koala already drains it after each script).
- Boa's startup-snapshot PR #3041 (2023) covered only startup state and
  was closed in December 2025 as "very outdated"; issue #1372 suggests a
  new collector would be needed.
- Koala's side is cheap: `DomTree` derives `Clone`. The JS bridge needs
  the timer scheduler moved from a thread-local onto `JsRuntime`, and the
  `NodeId` to `JsObject` wrapper map remapped after a clone.

Routes that work:

| Route | Verdict |
|---|---|
| Boa in-process clone | Major work; a permanent fork of Boa |
| OS `fork()` of the current app | Not feasible: UI, loader and reqwest threads; macOS frameworks |
| Fork-server: one single-threaded process per page, forked at a quiet point | Feasible now, moderate work; copy-on-write, estimated milliseconds (unmeasured) |
| Koala's own JS engine with an index-based arena heap, natives and jobs as plain data | Feasible by design; fork is copying the arenas |
| MicroVM snapshots | Linux only, heavy |
| Record and replay | Cost grows with the history |

Every route copies only client state. Orders placed and emails sent stay
done.

### Who needs a fork, and how much

- Search agents backtrack by relaunching the browser and replaying every
  action from the start, with fixed 2.5 s sleeps per step (Koh et al.
  2024, `run.py`). ExACT (Oct 2024) measured 33.5 min per task for that
  agent, cut to 10 min with caching, and replay drift (element ids that
  no longer match) in 40% of tasks, cut to 10% by a word-overlap
  heuristic. Search is capped at depth 5 and 20 nodes "due to compute and
  budget limitations".
- Live-site search has been largely abandoned for safety: Agent Q,
  WebDreamer and Koh all restrict exploration to non-destructive actions
  or self-hosted environments; WebDreamer simulates outcomes with an LLM
  instead of exploring.
- RL infrastructure reports mixed pressure. WebServ (Oct 2025) and
  DeltaBox (May 2026, 14 ms checkpoint, 5 ms rollback, no browser
  support) attack environment cost; DeltaBox's MCTS workloads roll back
  49 times per 100 expansions once it is cheap. CUA-Gym (May 2026) and
  BrowserForge (Aug 2026) say rollout latency is not their bottleneck.
- No browser forks DOM and JS state today; the nearest are VM and CRIU
  snapshots at hundreds of milliseconds to seconds.
- For LLM-driven agents the browser is cheap next to inference (about
  $0.025 of browser time for a 15-minute task at $0.10/hour). Browser
  weight matters where there is no LLM call per page: crawling, data
  collection, and RL host density.
- Research agents mostly do not run JavaScript at all: no JS execution
  across 500M+ GPTBot fetches (Vercel/MERJ, Dec 2024). Content
  extractors reach F1 0.93 on articles but 0.41-0.84 on forums, products,
  listings and docs (WCXB, May 2026), where layout could help (untested).
  The dominant failure observed is bot blocking, which a new engine makes
  worse.

### The concrete opportunity

A browser for web-agent search, RL and evaluation over fixed,
self-hosted environments: fork a page's whole client state in
milliseconds, deterministic, many sessions per host, running the
standard benchmark apps.

- Structural: in-process fork (no browser offers it), determinism
  (Koala owns timers, network and layout), density as a library.
- Compatibility target is closed and known: WebShop, and WebArena /
  VisualWebArena's Magento storefront and admin, Postmill, GitLab,
  OSClass and OpenStreetMap. How JS-heavy each is was not checked.
- What it buys: removes relaunch, replay, sleeps and the client half of
  replay drift. Server-side state still drifts within a task, as it does
  today.

Against: the population is mostly research labs and RL teams, not
production agents; world-model approaches like WebDreamer avoid real
exploration; some RL teams say the environment is not their bottleneck.

A second, weaker opening: layout-aware extraction at crawl scale on
structured pages, limited by bot blocking.

## Checks before committing (2026-10-16)

### Fork timing, measured

A single-threaded process holding a page parsed, styled and laid out by
`koala_browser::parse_html_string`, plus a live `JsRuntime` over its DOM
with app-like state, forked with `libc::fork` (macOS, release build,
medians of 20; program in `tmp/fork-check/fork_check.rs`):

| Page and JS state | Reload | `fork()` returns | Fork to child's answer |
|---|---|---|---|
| Hacker News, none | 3.9 ms | 0.58 ms | 1.6 ms |
| Wikipedia, none (109 MB RSS) | 176 ms | 1.3 ms | 5.0 ms |
| Wikipedia, 100k objects (147 MB) | 162 ms + 149 ms state | 2.1 ms | 4.2 ms |
| Wikipedia, 1M objects (559 MB) | 172 ms + 1.5 s state | 11 ms | 13 ms |

Each child read and changed the inherited JS heap and DOM; the parent's
state stayed unchanged. "Reload" excludes the network and any agent
history, so real replay costs more. Not measured: copy-on-write cost as
children write (a GC pass could copy much of the heap), and Linux.

### Forking Chrome today

- Morph Cloud sells "Infinibranch Browsers": live-branching running
  Chromium sessions, $0.07 per browser-hour. Its only latency claim is a
  general "under 250 ms" for VM snapshot and branch; no browser
  benchmark, and the browser product page now returns 404.
- Daytona, E2B and CodeSandbox clone running VMs (0.25-3 s claimed; E2B
  pauses at about 4 s per GiB) and publish no browser numbers.
- CRIU on Chrome: a public attempt (criu#2365) dumped only after a
  patch and then failed to restore.
- DeltaBox forks in about 2 ms but requires a single-threaded,
  socket-free process, which Chrome is not.

So "no browser offers forking" holds literally, but a VM-level product
already sells the idea. Koala's defensible claim is cost per branch:
milliseconds with shared memory, against hundreds of milliseconds and
gigabytes per copy. It matters for wide fan-out (DeltaBox measured GPU
occupancy of 29-36% with slow forks against 95-97% with fast ones), less
for an agent waiting seconds on each model call.

### Benchmark apps in Koala

Public instances of the WebArena / WebShop apps, against Chrome with and
without JavaScript (`tmp/compat-check/`): of 14 pages, none usable, 4
partly usable, 8 unusable, 2 crashes. Chrome without JavaScript renders
Magento, Osclass, Postmill, WebShop, Wikipedia and OpenStreetMap's info
panel fully, so most failures are CSS and parser bugs, not missing JS:
percentage `font-size` computing to 0, `rem`, dropped `@media` and
`@font-face`, and a parser panic on `<caption>`. GitLab issue pages and
the OSM map are rendered by scripts and need fetch, a larger DOM API and
scripts running after load. Every benchmark task also needs clicking,
typing and form submission, none of which Koala supports.

## Verdict

"Branch the web" is worth pursuing, with a narrower first target than
the full WebArena suite.

- The capability is real and measured: 1-11 ms forks of a page with its
  JS heap, against a best competitor claim of 250 ms for whole-VM
  Chrome clones.
- The compatibility gap is in two parts. Reading the server-rendered
  apps is weeks of bounded CSS and parser work, which every positioning
  needs anyway. Operating them needs interaction: scripts that keep
  running after load, events, form input and submission, navigation.
- WebShop is the right first end-to-end target: server-rendered HTML,
  plain links and forms, no client-side app, and LATS already uses it
  as its web environment for tree search.

Order of work:

1. The CSS and parser fixes above (benefit everything).
2. Interaction: keep the JS runtime alive after load, form input and
   submission, link and form navigation.
3. A fork-server mode: one single-threaded page process, forked on
   request, behind a `reset` / `step` / `fork` interface.
4. Run a LATS-style search agent on WebShop with forking against its
   replay baseline, and publish the time and drift numbers. Benchmark
   against Morph if access allows.

## Trade-offs

- The differentiator only matters on pages Koala renders correctly, and
  every misrendered page is a failure Chrome does not have. Web
  compatibility stays the limiting factor for everything above.
- Chrome could ship a visibility-filtered snapshot from primitives it
  already has. Koala's edge is exactness and atomicity, not an
  impossible capability, and it lasts only until a CDP-level tool does
  the same over Chrome.
- Dropping hidden content can hide what a screen-reader user would get;
  the filtered tree must be the default for agents, never the only tree.

## Sources

- Koh et al., Tree Search for LM Agents (2024-07): https://arxiv.org/abs/2407.01476 ; https://github.com/kohjingyu/search-agents
- ExACT / R-MCTS (2024-10): https://arxiv.org/abs/2410.02052
- Agent Q (2024-08): https://arxiv.org/abs/2408.07199
- WebDreamer (2024-11): https://arxiv.org/abs/2411.06559
- WebServ (2025-10): https://arxiv.org/abs/2510.16252
- DeltaBox (2026-05): https://arxiv.org/abs/2605.22781
- CUA-Gym (2026-05): https://arxiv.org/abs/2605.25624
- BrowserForge (2026-08): https://arxiv.org/abs/2608.24848
- WCXB (2026-05): https://arxiv.org/abs/2605.21097
- Boa startup snapshot PR #3041 and issue #1372: https://github.com/boa-dev/boa/pull/3041 ; https://github.com/boa-dev/boa/issues/1372

- Chrome DevTools MCP tool reference: https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/tool-reference.md
- Playwright MCP README and issues #395, #889: https://github.com/microsoft/playwright-mcp
- Token measurements: https://itnext.io/why-does-playwright-mcp-use-so-many-tokens-da56b6dbd2db ; https://current.tinyfish.ai/tinyfish/issue/34/foundations/article/24492
- ComponentBench (2026-08-18): https://arxiv.org/abs/2608.18307
- Claude for Chrome (2025-08-25): https://claude.com/blog/claude-for-chrome
- TRAP (Dec 2025): https://arxiv.org/html/2512.23128v1
- CUA-HandCrafted (Jun 2026): https://arxiv.org/abs/2606.05233
- WASP (2025-04-22): https://arxiv.org/abs/2504.18575
- Brave on Comet (2025-08-20, 2025-10-21): https://brave.com/blog/comet-prompt-injection/ ; https://brave.com/blog/unseeable-prompt-injections/
- OpenAI on Atlas (2025-12-22): https://openai.com/index/hardening-atlas-against-prompt-injection
- Online-Mind2Web (2025-04): https://arxiv.org/abs/2504.01382
- WABER (2025-04): https://www.microsoft.com/en-us/research/wp-content/uploads/2025/04/134_WABER_Web_Agent_Benchmarki.pdf
- Independent stealth survey (2026-04-23): https://incoherency.co.uk/blog/stories/stealth-browser-survey-april-2026.html
- Lightpanda repo, benchmarks, issues #2140, #2343, #3921: https://github.com/lightpanda-io/browser ; https://lightpanda.io/docs/core-concepts/benchmarks
- Obscura: https://github.com/h4ckf0r0day/obscura
- Servo 0.1.0 (2026-04-13): https://servo.org/blog/2026/04/13/servo-0.1.0-release/
- IntersectionObserver v2: https://web.dev/articles/intersectionobserver-v2
- Removing old headless (M132): https://developer.chrome.com/blog/removing-headless-old-from-chrome
- Chromium process model: https://chromium.googlesource.com/chromium/src/+/main/docs/process_model_and_site_isolation.md
- Kernel images: https://github.com/kernel/kernel-images
