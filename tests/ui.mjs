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
const manifest = readFileSync(process.env.REMISE_MANIFEST ?? join(ROOT, "src-tauri/target/manifest.json"), "utf8");
const exe =
  process.env.CHROMIUM ??
  ["/usr/bin/chromium", "/usr/bin/google-chrome", "/usr/bin/chromium-browser"].find((p) => existsSync(p));
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2" };

const mock = `
window.__listeners = {};
window.__TAURI__ = {
  core: { invoke: async (cmd, args) => (cmd === "list_tools" ? ${manifest} : window.__run(args)) },
  event: { listen: async (name, cb) => { window.__listeners[name] = cb; return () => {}; } },
  webview: { getCurrentWebview: () => ({ onDragDropEvent: async (cb) => { window.__drag = cb; } }) },
  dialog: { open: async () => null },
  opener: { revealItemInDir: async (p) => { window.__revealed = p; } },
};
window.__runs = [];
window.__run = (args) => new Promise((res, rej) => {
  window.__runs.push(args);
  let p = 0;
  const t = setInterval(() => {
    p += 0.25;
    window.__listeners.progress({ payload: { job: args.job, step: 0, steps: args.steps.length, pct: p } });
    if (p < 1) return;
    clearInterval(t);
    if (args.inputs[0].includes("broken")) rej("gs: Error: /syntaxerror");
    else res([args.inputs[0].replace(/\\.(\\w+)$/, ".out.$1")]);
  }, 60);
});
window.__drop = (paths) => {
  window.__drag({ payload: { type: "enter", paths } });
  window.__drag({ payload: { type: "drop", paths } });
};
`;

async function open(scheme) {
  const browser = await chromium.launch({ executablePath: exe });
  const page = await browser.newPage({ viewport: { width: 1180, height: 780 }, colorScheme: scheme });
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
}

for (const scheme of ["light", "dark"]) {
  const { browser, page, errors, shot } = await open(scheme);
  await shot("wall");
  await toolFlow(page, shot);
  await wallFlow(page, shot);
  await chainFlow(page, shot);
  await browser.close();
  assert.deepEqual(errors, [], "no console errors");
  console.log(`ui ${scheme}: ok`);
}
