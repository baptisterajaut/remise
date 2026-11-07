// Headless walk-through of the real frontend against a mocked Tauri bridge.
// The tool manifest is the real one (dumped by `cargo test manifest`); runs are simulated,
// the runner itself is covered by the Rust smoke test.
//   pnpm test:ui                 (CHROMIUM=/path/to/chrome to override, SHOTS=dir for screenshots)
import { chromium } from "playwright-core";
import { readFileSync, existsSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
// Every tool marked as installed: the walk-through tests the UI, not this machine's binaries.
const manifest = JSON.stringify(
  JSON.parse(readFileSync(process.env.REMISE_MANIFEST ?? join(ROOT, "src-tauri/target/manifest.json"), "utf8")).map((t) => ({ ...t, missing: [] })),
);
const exe =
  process.env.CHROMIUM ??
  ["/usr/bin/chromium", "/usr/bin/google-chrome", "/usr/bin/chromium-browser"].find((p) => existsSync(p));
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2" };

const mock = `
window.__listeners = {};
window.__TAURI__ = {
  core: {
    invoke: async (cmd, args) => {
      if (cmd === "list_tools") return ${manifest}.map((t) => ({ ...t, missing: window.__missing[t.id] ?? [] }));
      if (cmd === "downloads") return { chromium: { name: "Chromium", mb: 129 } };
      if (cmd === "download") return window.__download(args);
      if (cmd === "stop") return window.__pending[args.job]?.("Stopped.");
      if (cmd === "preview") {
        if (args.params.pattern.includes("/")) throw "the pattern is a file name: no slashes, no ..";
        return args.inputs.map((p, i) => [p.split("/").pop(), args.params.pattern.replace("{n}", String(i + 1).padStart(2, "0")) + ".jpg"]);
      }
      return window.__run(args);
    },
  },
  event: { listen: async (name, cb) => { window.__listeners[name] = cb; return () => {}; } },
  webview: { getCurrentWebview: () => ({ onDragDropEvent: async (cb) => { window.__drag = cb; } }) },
  dialog: { open: async () => null },
  opener: { revealItemInDir: async (p) => { window.__revealed = p; } },
};
// HTML to PDF starts without a browser, which Remise can download.
// A test can set what is missing for the next page load.
window.__missing = JSON.parse(sessionStorage.getItem("missing")) ?? { "pdf-html": ["chromium"] };
window.__downloads = [];
window.__download = ({ bin }) => new Promise((res, rej) => {
  if (window.__offline) return rej("curl: (6) Could not resolve host");
  window.__downloads.push(bin);
  window.__listeners.progress({ payload: { job: "dl-" + bin, step: 0, steps: 1, pct: 0.5 } });
  setTimeout(() => { window.__missing = {}; res(); }, 150);
});
window.__runs = [];
window.__pending = {};
window.__run = (args) => new Promise((res, rej) => {
  window.__runs.push(args);
  // A "slow" file only ends when stopped.
  if ((args.inputs[0] ?? "").includes("slow")) return (window.__pending[args.job] = rej);
  let p = 0;
  const t = setInterval(() => {
    p += 0.25;
    window.__listeners.progress({ payload: { job: args.job, step: 0, steps: args.steps.length, pct: p } });
    if (p < 1) return;
    clearInterval(t);
    const first = args.inputs[0] ?? "/Downloads/remise.made.png";
    if (first.includes("broken")) rej("gs: Error: /syntaxerror");
    else res([first.replace(/\\.(\\w+)$/, ".out.$1")]);
  }, 60);
});
window.__drop = (paths) => {
  window.__drag({ payload: { type: "enter", paths } });
  window.__drag({ payload: { type: "drop", paths } });
};
`;

async function open(scheme) {
  const browser = await chromium.launch({ executablePath: exe });
  const page = await browser.newPage({ viewport: { width: 1180, height: 780 }, colorScheme: scheme, timezoneId: "Europe/Paris" });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await page.route("http://remise.test/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    const file = join(SRC, path === "/" ? "index.html" : path);
    route.fulfill({ body: readFileSync(file), contentType: TYPES[extname(file)] ?? "application/octet-stream" });
  });
  await page.addInitScript(mock);
  await page.goto("http://remise.test/");
  await page.waitForSelector(".hook");
  const shot = (n) => process.env.SHOTS && page.screenshot({ path: join(process.env.SHOTS, `${scheme}-${n}.png`) });
  return { browser, page, errors, shot };
}

async function toolFlow(page, shot) {
  await page.getByRole("button", { name: /Compress PDF/ }).click();
  await page.evaluate(() => window.__drop(["/d/Q3.pdf", "/d/broken.pdf", "/d/photo.jpg"]));
  assert.equal(await page.locator(".files li").count(), 2, "the jpg is skipped");
  await page.getByRole("button", { name: "Strong", exact: true }).click();
  assert.equal(await page.getByRole("button", { name: "Strong", exact: true }).getAttribute("aria-pressed"), "true");
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.waitForSelector(".files li.fail");
  await page.waitForSelector(".files li.done");
  await shot("bench");
  const run = await page.evaluate(() => window.__runs[0]);
  assert.deepEqual(run.steps, [{ tool: "pdf-compress", params: { level: "screen" } }]);
  assert.match(await page.locator(".files .err").textContent(), /syntaxerror/);
  await page.getByRole("button", { name: "show" }).click();
  assert.equal(await page.evaluate(() => window.__revealed), "/d/Q3.out.pdf");
}

// Hovering a tag must not replay its entrance (which starts hidden, after a per-tag delay).
async function hoverFlow(page) {
  await page.waitForFunction(() => document.getAnimations().every((a) => a.playState === "finished"));
  const last = page.locator(".wall .hook").last();
  await last.scrollIntoViewIfNeeded();
  await last.hover();
  await page.mouse.move(2, 2);
  await page.waitForTimeout(80);
  assert.equal(await last.evaluate((el) => getComputedStyle(el).opacity), "1", "a tag stays visible after the pointer leaves");
}

async function wallFlow(page, shot) {
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.__drop(["/d/clip.mp4"]));
  await shot("dropfilter");
  const names = await page.locator(".wall .hook h3").allTextContents();
  assert.ok(names.includes("Video to GIF") && !names.includes("Compress PDF"), "drop filters the wall");
  await page.getByRole("button", { name: "show all" }).click();
  await page.keyboard.press("/");
  await page.keyboard.type("to gif");
  assert.deepEqual(await page.locator(".wall .hook h3").allTextContents(), ["Video to GIF"]);
  await page.fill("#search", "");
}

async function chainFlow(page, shot) {
  await page.getByRole("button", { name: /Chains/ }).click();
  await page.fill("#chain-name", "Scan to archive");
  for (const id of ["pdf-compress", "pdf-grayscale", "pdf-web"]) {
    await page.selectOption("#chain-add", id);
    await page.click("#chain-add-btn");
  }
  const offered = await page.locator("#chain-add option").evaluateAll((os) => os.map((o) => o.value));
  assert.ok(!offered.includes("vid-gif"), "after a PDF step, video tools are not offered");
  await page.click("#chain-save");
  await shot("chains");
  await page.locator(".chain-list .hook").first().click();
  await page.evaluate(() => window.__drop(["/d/scan.pdf"]));
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.waitForSelector(".files li.done");
  const run = await page.evaluate(() => window.__runs.at(-1));
  assert.deepEqual(run.steps.map((s) => s.tool), ["pdf-compress", "pdf-grayscale", "pdf-web"]);

  // What may follow depends on what the previous step actually writes, not on its family.
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /Chains/ }).click();
  await page.selectOption("#chain-add", "vid-extract-audio");
  await page.click("#chain-add-btn");
  const next = await page.locator("#chain-add option").evaluateAll((os) => os.map((o) => o.value));
  assert.ok(next.includes("aud-compress") && !next.includes("pdf-compress"), "audio tools follow Extract audio");
}

async function generatorFlow(page, shot) {
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /^Create/ }).click();
  await page.getByRole("button", { name: /Gradient/ }).click();
  assert.ok(await page.locator("#drop").isHidden(), "generators have no drop zone");
  await page.getByRole("button", { name: "Radial", exact: true }).click();
  await page.getByRole("button", { name: "Make it", exact: true }).click();
  await page.waitForSelector(".files li.done");
  await shot("generator");
  const run = await page.evaluate(() => window.__runs.at(-1));
  assert.deepEqual(run.inputs, []);
  assert.equal(run.steps[0].tool, "create-gradient");
  assert.equal(run.steps[0].params.kind, "radial");
  // Dropped files never offer a generator.
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.__drop(["/d/photo.png"]));
  const names = await page.locator(".wall .hook h3").allTextContents();
  assert.ok(names.includes("Device mockup") && !names.includes("Gradient") && !names.includes("QR code"));
}

/** Back to the whole wall: no dropped-file filter, every family. */
async function resetWall(page) {
  await page.keyboard.press("Escape");
  const all = page.getByRole("button", { name: "show all" });
  if (await all.isVisible()) await all.click();
  await page.getByRole("button", { name: /^Everything/ }).click();
}

async function previewFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Batch rename/ }).click();
  await page.evaluate(() => window.__drop(["/d/a.jpg", "/d/b.jpg"]));
  await page.fill(".params input", "trip-{n}");
  await page.waitForFunction(() => document.querySelector(".preview li")?.textContent === "a.jpg → trip-01.jpg");
  await page.fill(".params input", "a/b");
  await page.waitForSelector(".files .err");
  assert.match(await page.locator(".files .err").textContent(), /no slashes/);
}

async function datesFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Change file dates/ }).click();
  await page.evaluate(() => window.__drop(["/d/a.txt"]));
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.waitForSelector(".files li.done");
  const params = await page.evaluate(() => window.__runs.at(-1).steps[0].params);
  assert.equal(params.tz, "60", "a January date in Paris is UTC+1");
}

/** A tool's own password is sent with the run and never saved with its other settings. */
async function archiveFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Create archive/ }).click();
  await page.getByRole("button", { name: "7z", exact: true }).click();
  await page.fill("#params input[type=password]", "s3cret");
  await page.evaluate(() => window.__drop(["/d/a.txt", "/d/b.txt"]));
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.waitForSelector(".files li.done");
  assert.deepEqual(await page.evaluate(() => window.__runs.at(-1).steps[0].params), { format: "7z", password: "s3cret" });
  const saved = await page.evaluate(() => localStorage.getItem("remise.params.arc-create"));
  assert.ok(saved.includes("7z") && !saved.includes("s3cret"), saved);
}

/** Newer tools: the card opens, an option sticks, and the run carries it. */
const NEW_TOOLS = [{ card: /PDF\/A/, option: "PDF/A-1b", drop: ["/d/a.pdf"], params: { level: "1" } }];
async function newToolsFlow(page) {
  for (const t of NEW_TOOLS) {
    await resetWall(page);
    await page.getByRole("button", { name: t.card }).click();
    await page.getByRole("button", { name: t.option, exact: true }).click();
    await page.evaluate((paths) => window.__drop(paths), t.drop);
    await page.getByRole("button", { name: "Run", exact: true }).click();
    await page.waitForSelector(".files li.done");
    assert.deepEqual(await page.evaluate(() => window.__runs.at(-1).steps[0].params), t.params, String(t.card));
  }
}

async function stopFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Compress video/ }).click();
  await page.evaluate(() => window.__drop(["/d/slow.mp4", "/d/next.mp4"]));
  const run = page.locator("#run");
  await run.click();
  await page.waitForFunction(() => document.getElementById("run").textContent === "Stop");
  await run.click();
  await page.waitForFunction(() => document.getElementById("run").textContent === "Run");
  assert.equal(await page.locator(".files li.done, .files li.fail").count(), 0, "nothing failed, nothing ran after");
  assert.ok(!(await page.evaluate(() => window.__runs.some((r) => r.inputs[0] === "/d/next.mp4"))), "the next file waits");
}

async function downloadFlow(page, shot) {
  await resetWall(page);
  await page.getByRole("button", { name: /HTML to PDF/ }).click();
  const run = page.locator("#run");
  assert.equal(await run.textContent(), "Download Chromium (129 MB)");
  // A failed download says why on the bench, not only in a passing toast.
  await page.evaluate(() => (window.__offline = true));
  await run.click();
  await page.waitForSelector("#bench-note.err");
  assert.match(await page.locator("#bench-note").textContent(), /Could not resolve host/);
  await page.evaluate(() => (window.__offline = false));
  await run.click();
  await page.waitForFunction(() => document.getElementById("run").textContent.startsWith("Downloading… 50"));
  await shot("downloading");
  await page.waitForFunction(() => document.getElementById("run").textContent === "Run");
  assert.deepEqual(await page.evaluate(() => window.__downloads), ["chromium"]);
  assert.ok(await page.locator("#bench-note").isHidden(), "the error goes once the download works");
}

const run = (page) => page.locator("#run");
const focused = (page) => page.evaluate(() => document.activeElement.closest("[data-id]")?.querySelector("h3")?.textContent ?? document.activeElement.id);

/** A running bench can't be left (its queue would leak into the next tool), nor stopped by a double click. */
async function busyFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Compress video/ }).click();
  await page.evaluate(() => window.__drop(["/d/slow.mp4", "/d/next.mp4"]));
  await run(page).dblclick();
  await page.waitForTimeout(150);
  assert.equal(await run(page).textContent(), "Stop", "the second click of a double click doesn't stop the run");
  assert.ok(await page.locator("#clear").isDisabled(), "Clear can't empty a running queue");
  await page.click("#back");
  await page.getByRole("button", { name: /^Image/ }).click();
  await page.fill("#search", "gif");
  assert.ok(await page.locator("#bench").isVisible(), "the bench stays while it runs");
  assert.equal(await page.inputValue("#search"), "");
  await run(page).click();
  await page.waitForFunction(() => document.getElementById("run").textContent === "Run");
  assert.ok(await page.evaluate(() => window.__runs.every((r) => r.steps[0].tool !== "img-convert")));
}

/** Keyboard: focus lands on the bench, Escape in a field stays, and going back returns to the card and the scroll. */
async function keysFlow(page) {
  await resetWall(page);
  const card = page.locator(".wall .hook").last();
  const name = await card.locator("h3").textContent();
  await card.focus();
  await page.keyboard.press("Enter");
  assert.equal(await focused(page), "bench-title", "opening a tool focuses its title");
  await page.keyboard.press("Escape");
  assert.equal(await focused(page), name, "back on the card it was opened from");
  assert.ok((await page.evaluate(() => document.getElementById("main").scrollTop)) > 0, "the wall keeps its scroll");

  await page.getByRole("button", { name: /Batch rename/ }).click();
  await page.focus(".params input");
  await page.keyboard.press("Escape");
  assert.ok(await page.locator("#bench").isVisible(), "Escape in a text field doesn't close the bench");

  await resetWall(page);
  await page.getByRole("button", { name: /Compress PDF/ }).click();
  await page.getByRole("button", { name: "Strong", exact: true }).focus();
  await page.keyboard.press("Space");
  assert.equal(await page.evaluate(() => document.activeElement.textContent), "Strong", "a choice keeps the focus");
  await page.getByRole("button", { name: /^Video/ }).focus();
  await page.keyboard.press("Enter");
  assert.equal(await page.evaluate(() => document.activeElement.dataset.focus), "video", "the rail keeps the focus");
}

/** A failed file dropped again goes back in the queue; search shows cards at once; a drop looks across families. */
async function wallAndRetryFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Compress PDF/ }).click();
  await page.evaluate(() => window.__drop(["/d/broken.pdf"]));
  await run(page).click();
  await page.waitForSelector(".files li.fail");
  await page.evaluate(() => window.__drop(["/d/broken.pdf"]));
  assert.equal(await page.locator(".files li.queued").count(), 1, "dropped again, it is queued again");
  assert.ok(await run(page).isEnabled());

  await resetWall(page);
  await page.fill("#search", "i");
  assert.equal(await page.locator(".wall .hook").last().evaluate((el) => getComputedStyle(el).opacity), "1", "search doesn't replay the hang-in");
  await page.fill("#search", "");
  await page.getByRole("button", { name: /^PDF/ }).click();
  await page.evaluate(() => window.__drop(["/d/clip.mp4"]));
  assert.ok((await page.locator(".wall .hook h3").allTextContents()).includes("Video to GIF"), "a drop ignores the family filter");
}

/** Chains: a password is asked for at run time and never saved; removing a step drops what no longer fits. */
async function chainEdgeFlow(page) {
  await resetWall(page);
  await page.getByRole("button", { name: /Chains/ }).click();
  while (await page.locator(".chain-draft .x").count()) await page.locator(".chain-draft .x").first().click();
  await page.fill("#chain-name", "Unlock and shrink");
  await page.selectOption("#chain-add", "pdf-unlock");
  await page.click("#chain-add-btn");
  assert.equal(await page.locator(".chain-draft input[type=password]").count(), 0, "no password field in the builder");
  await page.selectOption("#chain-add", "pdf-compress");
  await page.click("#chain-add-btn");
  await page.click("#chain-save");
  await page.locator(".chain-list .hook", { hasText: "Unlock and shrink" }).click();
  assert.equal(await page.locator(".files li").count(), 0, "a chain opens on an empty queue");
  await page.fill("#params input[type=password]", "s3cret");
  await page.evaluate(() => window.__drop(["/d/locked.pdf"]));
  await run(page).click();
  await page.waitForSelector(".files li.done");
  assert.equal(await page.evaluate(() => window.__runs.at(-1).steps[0].params.password), "s3cret");
  assert.ok(!(await page.evaluate(() => localStorage.getItem("remise.chains"))).includes("s3cret"));

  await page.keyboard.press("Escape");
  assert.ok(await page.locator("#chains").isVisible(), "a chain's bench goes back to the chains");
  for (const id of ["vid-compress", "vid-extract-audio", "aud-compress"]) {
    await page.selectOption("#chain-add", id);
    await page.click("#chain-add-btn");
  }
  await page.locator(".chain-draft .x").nth(1).click();
  assert.deepEqual(await page.locator(".chain-draft h4").allTextContents(), ["Compress video"], "audio steps can't follow a video");
  await page.locator(".chain-card .remove").first().click();
  await page.locator(".chain-card .remove").first().click();
  assert.equal(await page.locator(".chain-card").count(), 1, "remove asks for a second press");
}

/** A chain saved by another version, naming a tool this one lacks, is refused instead of crashing. */
async function staleChainFlow(page) {
  await page.evaluate(() =>
    localStorage.setItem("remise.chains", JSON.stringify([{ id: "cold", name: "Old", steps: [{ tool: "gone", params: {} }, { tool: "pdf-compress", params: {} }] }])),
  );
  await page.reload();
  await page.waitForSelector(".hook");
  await page.getByRole("button", { name: /Chains/ }).click();
  await page.locator(".chain-list .hook").click();
  assert.ok(await page.locator("#bench").isHidden());
  assert.match(await page.locator("#toast").textContent(), /gone/);

  // What Remise can't download is spelled out on the bench, not hidden in a tooltip.
  await page.evaluate(() => sessionStorage.setItem("missing", JSON.stringify({ "pdf-compress": ["gs"] })));
  await page.reload();
  await page.waitForSelector(".hook");
  await page.getByRole("button", { name: /Compress PDF/ }).click();
  assert.match(await page.locator("#bench-note").textContent(), /install ghostscript/);
  assert.equal(await page.locator("#run").textContent(), "Needs Ghostscript");
}

for (const scheme of ["light", "dark"]) {
  const { browser, page, errors, shot } = await open(scheme);
  await shot("wall");
  await hoverFlow(page);
  await toolFlow(page, shot);
  await wallFlow(page, shot);
  await chainFlow(page, shot);
  await generatorFlow(page, shot);
  await previewFlow(page);
  await datesFlow(page);
  await archiveFlow(page);
  await newToolsFlow(page);
  await stopFlow(page);
  await downloadFlow(page, shot);
  await busyFlow(page);
  await keysFlow(page);
  await wallAndRetryFlow(page);
  await chainEdgeFlow(page);
  await staleChainFlow(page);
  await browser.close();
  assert.deepEqual(errors, [], "no console errors");
  console.log(`ui ${scheme}: ok`);
}
