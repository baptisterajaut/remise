const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWebview } = window.__TAURI__.webview;
const { open: openDialog } = window.__TAURI__.dialog;
const { revealItemInDir } = window.__TAURI__.opener;

const FAMILIES = [
  ["all", "Everything", "var(--ink)"],
  ["pdf", "PDF", "var(--pdf)"],
  ["docs", "Docs", "var(--docs)"],
  ["image", "Image", "var(--image)"],
  ["video", "Video", "var(--video)"],
  ["audio", "Audio", "var(--audio)"],
  ["create", "Create", "var(--create)"],
  ["files", "Files", "var(--files)"],
];
const famColor = (f) => `var(--${f})`;

/** Binary → [product name, package name for brew/apt]. */
const PRODUCTS = {
  gs: ["Ghostscript", "ghostscript"],
  magick: ["ImageMagick", "imagemagick"],
  qpdf: ["qpdf", "qpdf"],
  ffmpeg: ["FFmpeg", "ffmpeg"],
  "7z": ["7-Zip", "7zip"],
  potrace: ["Potrace", "potrace"],
  onnxruntime: ["ONNX Runtime", null],
  pdfcraft: ["PdfCraft", null],
  chromium: ["a Chromium browser", null],
  "yt-dlp": ["yt-dlp", "yt-dlp"],
  "whisper-cli": ["whisper.cpp", null],
  "waifu2x-ncnn-vulkan": ["waifu2x", null],
  soffice: ["LibreOffice", null],
  pandoc: ["Pandoc", "pandoc"],
  typst: ["Typst", "typst"],
};
/** Optional tools that don't come from a package manager. */
const EXTRA_HINTS = {
  chromium: "Any of Google Chrome, Microsoft Edge or Chromium",
  "waifu2x-ncnn-vulkan": "waifu2x: github.com/nihui/waifu2x-ncnn-vulkan (unzip, keep models-upconv_7_photo/ next to it)",
};
const product = (bin) => PRODUCTS[bin]?.[0] ?? state.downloads[bin]?.name ?? bin;

/** Bins Remise can fetch itself (big browsers and models), by name: { name, mb }. */
const fetchable = (bins) => bins.length > 0 && bins.every((b) => state.downloads[b]);

function installHint(bins) {
  if (fetchable(bins)) return `Remise downloads it for you, once: ${bins.map((b) => `${state.downloads[b].name}, ${state.downloads[b].mb} MB`).join("; ")}.`;
  const pkgs = bins.map((b) => PRODUCTS[b]?.[1]).filter(Boolean);
  const extra = bins.map((b) => EXTRA_HINTS[b]).filter(Boolean);
  const mgr = /Mac/.test(navigator.userAgent) ? "brew install" : /Win/.test(navigator.userAgent) ? "winget install" : "sudo apt install";
  return [pkgs.length && `${mgr} ${pkgs.join(" ")}`, ...extra].filter(Boolean).join("\n");
}
const $ = (id) => document.getElementById(id);

const state = {
  tools: [],
  downloads: {},
  fetching: null, // { pct } while a download runs
  family: "all",
  query: "",
  dropped: null, // paths dropped on the wall, used to filter tools that fit
  view: "wall",
  bench: null, // { tool?, chain?, params, rows, running }
  chains: load("remise.chains", []),
  draft: [],
};

/* ───────────────────────── helpers ───────────────────────── */

/** Tiny element builder. Text only ever goes through textContent: file names are untrusted. */
function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "style") el.style.cssText = v;
    else if (k.startsWith("aria-")) el.setAttribute(k, String(v));
    else if (v !== false && v != null) el.setAttribute(k, v === true ? "" : v);
  }
  el.append(...kids.flat().filter((x) => x != null && x !== false));
  return el;
}

function load(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key)) ?? fallback;
  } catch {
    return fallback;
  }
}

function save(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private mode: preferences just won't stick */
  }
}

const base = (p) => p.split(/[\\/]/).pop();
const ext = (p) => {
  const b = base(p);
  return b.includes(".") ? b.split(".").pop().toLowerCase() : "";
};
const fits = (tool, paths) => !tool.generator && (!tool.accepts.length || paths.every((p) => tool.accepts.includes(ext(p))));
const toolById = (id) => state.tools.find((t) => t.id === id);

function defaults(tool) {
  const saved = load(`remise.params.${tool.id}`, {});
  return Object.fromEntries(tool.params.map((p) => [p.key, saved[p.key] ?? p.default]));
}

let toastTimer;
function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 2600);
}

/* ───────────────────────── rail & wall ───────────────────────── */

function renderRail() {
  const count = (f) => state.tools.filter((t) => f === "all" || t.family === f).length;
  const item = ([f, label, color]) =>
    h(
      "button",
      { type: "button", "aria-current": state.view === "wall" && state.family === f, onclick: () => pickFamily(f) },
      h("span", { class: "dot", style: `--c:${color}` }),
      label,
      h("span", { class: "n" }, String(count(f))),
    );
  const chains = h(
    "button",
    { type: "button", "aria-current": state.view === "chains", onclick: () => show("chains") },
    h("span", { class: "dot", style: "--c:var(--chain)" }),
    "Chains",
    h("span", { class: "n" }, String(state.chains.length)),
  );
  $("rail").replaceChildren(...FAMILIES.map(item), h("hr"), chains);
}

function pickFamily(f) {
  state.family = f;
  show("wall");
}

function visibleTools() {
  const q = state.query.trim().toLowerCase();
  return state.tools.filter(
    (t) =>
      (state.family === "all" || t.family === state.family) &&
      (!q || `${t.name} ${t.blurb} ${t.family}`.toLowerCase().includes(q)) &&
      (!state.dropped || fits(t, state.dropped)),
  );
}

/** Deterministic "hand-hung" tilt per tool. */
function tilt(id) {
  let n = 0;
  for (const c of id) n = (n * 31 + c.charCodeAt(0)) % 997;
  return ((n % 9) - 4) * 0.35;
}

function tag(tool, i) {
  const off = tool.missing.length > 0;
  return h(
    "button",
    {
      type: "button",
      class: off ? "hook off" : "hook",
      style: `--c:${famColor(tool.family)};--r:${tilt(tool.id)}deg;--i:${i}`,
      onclick: () => openTool(tool),
    },
    h(
      "div",
      { class: "tagbody" },
      tool.multi && h("span", { class: "multi" }, "many → 1"),
      h("span", { class: "fam" }, tool.family),
      h("h3", {}, tool.name),
      h("p", {}, tool.blurb),
      off && h("span", { class: "needs", title: installHint(tool.missing) }, `needs ${tool.missing.map(product).join(", ")}`),
    ),
  );
}

function droppedBanner() {
  const n = state.dropped.length;
  return h(
    "div",
    { class: "filtered" },
    `${n} file${n > 1 ? "s" : ""} in hand — showing what fits.`,
    h("button", { type: "button", onclick: () => ((state.dropped = null), renderWall()) }, "show all"),
  );
}

function renderWall() {
  const tools = visibleTools();
  const kids = tools.map(tag);
  if (state.dropped) kids.unshift(droppedBanner());
  if (!tools.length) kids.push(h("p", { class: "empty" }, "Nothing on the wall matches."));
  $("wall").replaceChildren(...kids);
}

/* ───────────────────────── params form ───────────────────────── */

function segmented(p, values, onChange) {
  const seg = h("div", { class: "seg" });
  const paint = () =>
    seg.replaceChildren(
      ...p.options.map(([v, label]) =>
        h("button", { type: "button", "aria-pressed": values[p.key] === v, onclick: () => (onChange(p.key, v), paint()) }, label),
      ),
    );
  paint();
  return seg;
}

function control(p, values, onChange) {
  if (p.kind === "text") {
    const type = p.key === "password" ? "password" : "text";
    return h("input", { type, value: values[p.key], placeholder: p.placeholder, oninput: (e) => onChange(p.key, e.target.value) });
  }
  if (p.options.length <= 4) return segmented(p, values, onChange);
  const sel = h(
    "select",
    { onchange: (e) => onChange(p.key, e.target.value) },
    p.options.map(([v, label]) => h("option", { value: v, selected: values[p.key] === v }, label)),
  );
  return sel;
}

/** A label wraps exactly one control; a row of toggle buttons is a named group instead. */
function field(p, values, onChange) {
  const ctl = control(p, values, onChange);
  if (ctl.classList.contains("seg")) return h("div", { class: "field", role: "group", "aria-label": p.label }, h("span", {}, p.label), ctl);
  return h("label", { class: "field" }, p.label, ctl);
}

function paramsForm(tool, values, onChange) {
  return tool.params.map((p) => field(p, values, onChange));
}

/* ───────────────────────── bench (one tool or one chain) ───────────────────────── */

function openTool(tool, files = state.dropped ?? []) {
  const params = defaults(tool);
  state.bench = { tool, params, rows: [], running: false };
  state.dropped = null;
  show("bench");
  $("bench-family").textContent = tool.family;
  $("bench-title").textContent = tool.name;
  $("bench-blurb").textContent = tool.blurb;
  $("bench").style.setProperty("--c", famColor(tool.family));
  $("chain-steps").hidden = true;
  const onChange = (k, v) => {
    params[k] = v;
    if (k !== "password") save(`remise.params.${tool.id}`, { ...load(`remise.params.${tool.id}`, {}), [k]: v });
  };
  $("params").replaceChildren(...paramsForm(tool, params, onChange));
  $("accepts").textContent = tool.accepts.length ? tool.accepts.join(" · ") : "any file or folder";
  // Generators make something from their settings alone: no drop zone, a Make button.
  $("drop").hidden = $("clear").hidden = tool.generator;
  if (!tool.generator) addFiles(files);
  else renderRows();
}

function openChain(chain) {
  state.bench = { chain, rows: [], running: false };
  show("bench");
  $("bench-family").textContent = "chain";
  $("bench-title").textContent = chain.name;
  $("bench-blurb").textContent = `${chain.steps.length} steps, one result per file.`;
  $("bench").style.setProperty("--c", "var(--chain)");
  $("params").replaceChildren();
  const first = toolById(chain.steps[0].tool);
  $("accepts").textContent = first?.accepts.join(" · ") ?? "";
  $("chain-steps").hidden = false;
  $("drop").hidden = $("clear").hidden = false;
  $("chain-steps").replaceChildren(...chain.steps.map(stepSummary));
}

function stepSummary(step) {
  const tool = toolById(step.tool);
  const opts = tool.params.map((p) => {
    const v = step.params[p.key];
    return p.kind === "select" ? (p.options.find(([o]) => o === v)?.[1] ?? v) : v;
  });
  return h("div", { style: `--c:${famColor(tool.family)}` }, h("h4", {}, tool.name), h("p", { class: "accepts" }, opts.join(" · ")));
}

function benchTools() {
  const b = state.bench;
  return b.tool ? [b.tool] : b.chain.steps.map((s) => toolById(s.tool));
}

function addFiles(paths) {
  const b = state.bench;
  const first = benchTools()[0];
  const ok = paths.filter((p) => fits(first, [p]));
  if (ok.length < paths.length) toast(`${paths.length - ok.length} file(s) skipped — ${first.name} doesn't take them`);
  const known = new Set(b.rows.flatMap((r) => r.paths));
  const fresh = ok.filter((p) => !known.has(p));
  if (b.tool?.multi) {
    const pending = b.rows.find((r) => r.state === "queued");
    if (pending) pending.paths.push(...fresh);
    else if (fresh.length) b.rows.push(row(fresh));
  } else {
    b.rows.push(...fresh.map((p) => row([p])));
  }
  renderRows();
}

let jobSeq = 0;
const row = (paths) => ({ id: `j${++jobSeq}`, paths, state: "queued", pct: null, step: 0, out: [], err: "" });

function rowLabel(r) {
  if (!r.paths.length) return state.bench.tool.name;
  return r.paths.length > 1 ? `${base(r.paths[0])} + ${r.paths.length - 1} more` : base(r.paths[0]);
}

function rowStatus(r) {
  const steps = benchTools().length;
  if (r.state === "running") {
    const step = steps > 1 ? `step ${r.step + 1}/${steps} · ` : "";
    return `${step}${r.pct == null ? "working…" : `${Math.round(r.pct * 100)}%`}`;
  }
  return { queued: "ready", done: "done", fail: "failed" }[r.state];
}

function rowView(r) {
  return h(
    "li",
    { class: r.state },
    h("span", { class: "name", title: r.paths.join("\n") }, rowLabel(r)),
    h("span", { class: "st" }, rowStatus(r)),
    r.state === "running" && h("div", { class: r.pct == null ? "bar indet" : "bar", style: `--p:${r.pct ?? 0}` }, h("i")),
    r.state === "fail" && h("pre", { class: "err" }, r.err),
    r.out.map((o) =>
      h("div", { class: "out" }, "→", h("span", { title: o }, base(o)), h("button", { type: "button", onclick: () => revealItemInDir(o) }, "show")),
    ),
  );
}

function renderRows() {
  const b = state.bench;
  $("files").replaceChildren(...b.rows.map(rowView));
  const missing = [...new Set(benchTools().flatMap((t) => t.missing))];
  const run = $("run");
  const ready = b.tool?.generator || b.rows.some((r) => r.state === "queued");
  run.disabled = Boolean(state.fetching) || b.stopping || (!b.running && (missing.length ? !fetchable(missing) : !ready));
  run.textContent = runLabel(missing);
  run.title = missing.length ? installHint(missing) : "";
}

function runLabel(missing) {
  if (state.fetching) return `Downloading… ${Math.round((state.fetching.pct ?? 0) * 100)} %`;
  if (fetchable(missing)) return `Download ${missing.map((m) => state.downloads[m].name).join(" + ")} (${missing.reduce((n, m) => n + state.downloads[m].mb, 0)} MB)`;
  if (missing.length) return `Needs ${missing.map(product).join(", ")}`;
  if (state.bench.stopping) return "Stopping…";
  if (state.bench.running) return "Stop";
  return state.bench.tool?.generator ? "Make it" : "Run";
}

/** Fetch what the open tool is missing, then pick the tools up again with it found. */
async function fetchMissing() {
  const b = state.bench;
  state.fetching = { pct: 0 };
  renderRows();
  try {
    for (const bin of new Set(benchTools().flatMap((t) => t.missing))) await invoke("download", { bin });
    toast("Downloaded. It stays on this computer, no need to do it again.");
  } catch (e) {
    toast(String(e));
  }
  state.fetching = null;
  state.tools = await invoke("list_tools");
  if (b.tool) b.tool = toolById(b.tool.id);
  renderRows();
}

async function runRow(r) {
  const b = state.bench;
  const steps = b.tool ? [{ tool: b.tool.id, params: b.params }] : b.chain.steps;
  r.state = "running";
  renderRows();
  try {
    r.out = await invoke("run", { job: r.id, steps, inputs: r.paths });
    r.state = "done";
  } catch (e) {
    r.state = "fail";
    r.err = String(e);
    if (r.err === "Stopped.") unstop(r);
  }
  renderRows();
}

/** A stopped file goes back in the queue; a stopped generator run just goes away. */
function unstop(r) {
  const b = state.bench;
  if (!r.paths.length) b.rows.splice(b.rows.indexOf(r), 1);
  Object.assign(r, { state: "queued", err: null, pct: null });
}

function stopRun() {
  const b = state.bench;
  b.stopping = true;
  const r = b.rows.find((x) => x.state === "running");
  if (r) invoke("stop", { job: r.id });
  renderRows();
}

async function runAll() {
  const b = state.bench;
  if (b.running) return stopRun();
  if (benchTools().some((t) => t.missing.length)) return fetchMissing();
  if (b.tool?.generator) b.rows.push(row([]));
  b.running = true;
  // Sequential on purpose: these tools already use every core.
  for (const r of b.rows.filter((x) => x.state === "queued")) {
    if (b.stopping) break;
    await runRow(r);
  }
  const stopped = b.stopping;
  b.running = b.stopping = false;
  renderRows();
  const failed = b.rows.filter((r) => r.state === "fail").length;
  if (stopped) toast("Stopped. What was left is still queued; press Run to go on.");
  else toast(failed ? `${failed} failed — details under each file` : "All done. Originals untouched.");
}

function onProgress({ payload }) {
  if (payload.job.startsWith("dl-")) {
    if (state.fetching) state.fetching.pct = payload.pct;
    return state.bench && renderRows();
  }
  const r = state.bench?.rows.find((x) => x.id === payload.job);
  if (!r || r.state !== "running") return;
  r.step = payload.step;
  r.pct = payload.pct;
  renderRows();
}

/* ───────────────────────── chains ───────────────────────── */

function chainable(tool) {
  return !tool.multi && !tool.generator && tool.output !== "none";
}

function renderDraft() {
  const items = state.draft.map((step, i) => {
    const tool = toolById(step.tool);
    const onChange = (k, v) => (step.params[k] = v);
    const x = h("button", { type: "button", class: "x", "aria-label": "Remove step", onclick: () => (state.draft.splice(i, 1), renderDraft()) }, "×");
    return h(
      "li",
      { style: `--c:${famColor(tool.family)}` },
      x,
      h("h4", {}, tool.name),
      h("div", { class: "params" }, paramsForm(tool, step.params, onChange)),
    );
  });
  $("chain-draft").replaceChildren(...items);
  renderAddStep();
}

/** Only offer tools that can take what the previous step outputs. */
function renderAddStep() {
  const last = toolById(state.draft.at(-1)?.tool);
  const options = state.tools.filter(chainable).filter((t) => !last || (last.output === "file" && takesOutputOf(t, last)));
  $("chain-add").replaceChildren(...options.map((t) => h("option", { value: t.id }, `${t.family} · ${t.name}`)));
  $("chain-add-btn").disabled = !options.length;
}

/** `produces` is the previous tool's output extension with default params. */
function takesOutputOf(next, prev) {
  return !next.accepts.length || next.accepts.includes(prev.produces);
}

function addStep() {
  const tool = toolById($("chain-add").value);
  if (tool) state.draft.push({ tool: tool.id, params: defaults(tool) });
  renderDraft();
}

function saveChain() {
  const name = $("chain-name").value.trim();
  if (!name || state.draft.length < 2) return toast("A chain needs a name and at least two steps");
  // Passwords stay out of storage: a saved chain asks again.
  const steps = state.draft.map((s) => ({ ...s, params: Object.fromEntries(Object.entries(s.params).filter(([k]) => k !== "password")) }));
  state.chains.push({ id: `c${Date.now().toString(36)}`, name, steps });
  save("remise.chains", state.chains);
  state.draft = [];
  $("chain-name").value = "";
  renderDraft();
  renderChains();
  renderRail();
}

function chainCard(chain, i) {
  const route = chain.steps.map((s) => toolById(s.tool)?.name ?? s.tool).join(" → ");
  const del = (e) => {
    e.stopPropagation();
    state.chains = state.chains.filter((c) => c.id !== chain.id);
    save("remise.chains", state.chains);
    renderChains();
    renderRail();
  };
  return h(
    "button",
    { type: "button", class: "hook", style: `--r:${tilt(chain.id)}deg;--i:${i}`, onclick: () => openChain(chain) },
    h(
      "div",
      { class: "tagbody" },
      h("span", { class: "fam" }, "chain"),
      h("h3", {}, chain.name),
      h("p", { class: "route" }, route),
      h("span", { class: "needs", onclick: del }, "remove"),
    ),
  );
}

function renderChains() {
  const cards = state.chains.map(chainCard);
  if (!cards.length) cards.push(h("p", { class: "hint" }, "No chains yet. Build one on the left — e.g. Compress PDF → Grayscale → Optimize for web."));
  $("chain-list").replaceChildren(...cards);
}

/* ───────────────────────── views & drag-drop ───────────────────────── */

function show(view) {
  state.view = view;
  $("wall").hidden = view !== "wall";
  $("bench").hidden = view !== "bench";
  $("chains").hidden = view !== "chains";
  if (view === "wall") renderWall();
  if (view === "chains") (renderDraft(), renderChains());
  $("main").scrollTop = 0;
  renderRail();
}

function onDrag({ payload }) {
  const hot = payload.type === "enter" || payload.type === "over";
  if (state.view === "bench") $("drop").classList.toggle("hot", hot);
  else $("dropveil").hidden = !hot || state.view !== "wall";
  if (payload.type !== "drop") return;
  $("drop").classList.remove("hot");
  $("dropveil").hidden = true;
  onDrop(payload.paths);
}

function onDrop(paths) {
  if (state.view === "bench" && !state.bench.running) return addFiles(paths);
  if (state.view !== "wall") return toast("Open a tool or a chain, then drop");
  state.dropped = paths;
  const fitting = visibleTools();
  if (fitting.length === 1) return openTool(fitting[0], paths);
  renderWall();
}

async function browse() {
  const picked = await openDialog({ multiple: true, directory: false });
  if (picked) addFiles([].concat(picked));
}

function onKey(e) {
  if (e.key === "/" && document.activeElement.tagName !== "INPUT") {
    e.preventDefault();
    $("search").focus();
  } else if (e.key === "Escape" && state.view !== "wall" && !state.bench?.running) {
    show("wall");
  }
}

async function init() {
  state.tools = await invoke("list_tools");
  state.downloads = await invoke("downloads");
  $("tool-count").textContent = state.tools.length;
  $("search").addEventListener("input", (e) => {
    state.query = e.target.value;
    if (state.view !== "wall") show("wall");
    else renderWall();
  });
  $("back").addEventListener("click", () => show(state.bench?.chain ? "chains" : "wall"));
  $("browse").addEventListener("click", browse);
  $("run").addEventListener("click", runAll);
  $("clear").addEventListener("click", () => {
    if (state.bench.running) return;
    state.bench.rows = [];
    renderRows();
  });
  $("chain-add-btn").addEventListener("click", addStep);
  $("chain-save").addEventListener("click", saveChain);
  document.addEventListener("keydown", onKey);
  await getCurrentWebview().onDragDropEvent(onDrag);
  await listen("progress", onProgress);
  show("wall");
}

init();
