// README screenshots from the real UI with a staged, realistic session (mocked Tauri bridge).
//   REMISE_MANIFEST=src-tauri/target/manifest.json node scripts/screenshots.mjs
import { chromium } from "playwright-core";
import { readFileSync, existsSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src");
const OUT = join(ROOT, "docs");
const tools = JSON.parse(readFileSync(process.env.REMISE_MANIFEST ?? join(ROOT, "src-tauri/target/manifest.json"), "utf8"));
const manifest = JSON.stringify(tools.map((t) => ({ ...t, missing: [] })));
const exe = process.env.CHROMIUM ?? ["/usr/bin/chromium", "/usr/bin/google-chrome"].find((p) => existsSync(p));
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2" };
const HOME = "/Users/sam/Videos";

// Runs: the first file finishes, the second sits at 64 %, the rest wait their turn.
const mock = `
window.__listeners = {};
let n = 0;
window.__TAURI__ = {
  core: { invoke: async (cmd, args) => {
    if (cmd === "list_tools") return ${manifest};
    if (cmd === "downloads") return {};
    const i = n++;
    if (i === 0) return [args.inputs[0].replace(/\\.(\\w+)$/, ".compressed.mp4")];
    setTimeout(() => window.__listeners.progress({ payload: { job: args.job, step: 0, steps: 1, pct: 0.64 } }), 50);
    return new Promise(() => {});
  } },
  event: { listen: async (name, cb) => { window.__listeners[name] = cb; return () => {}; } },
  webview: { getCurrentWebview: () => ({ onDragDropEvent: async (cb) => { window.__drag = cb; } }) },
  dialog: { open: async () => null },
  opener: { revealItemInDir: async () => {} },
};
window.__drop = (paths) => window.__drag({ payload: { type: "drop", paths } });
localStorage.setItem("remise.chains", JSON.stringify([
  { id: "c1", name: "Scan to archive", steps: [
    { tool: "pdf-ocr", params: { lang: "eng+fra" } },
    { tool: "pdf-compress", params: { level: "ebook" } },
    { tool: "pdf-web", params: {} } ] },
  { id: "c2", name: "Phone clip to story", steps: [
    { tool: "vid-crop", params: { ratio: "9/16" } },
    { tool: "vid-compress", params: { crf: "28" } } ] },
  { id: "c3", name: "Web-ready photos", steps: [
    { tool: "img-resize", params: { size: "1920x1920>" } },
    { tool: "img-strip", params: {} },
    { tool: "img-convert", params: { format: "webp" } } ] },
]));
`;

async function page(browser, scheme) {
  const p = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: scheme });
  await p.route("http://remise.test/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    const file = join(SRC, path === "/" ? "index.html" : path);
    route.fulfill({ body: readFileSync(file), contentType: TYPES[extname(file)] ?? "application/octet-stream" });
  });
  await p.addInitScript(mock);
  await p.goto("http://remise.test/");
  await p.waitForSelector(".hook");
  await p.waitForTimeout(1200); // let the tags finish hanging
  return p;
}

const browser = await chromium.launch({ executablePath: exe });

// Crop at the last row of tags that is fully in view.
async function shootWall(p, name) {
  const bottom = await p.evaluate(() => {
    const h = window.innerHeight;
    const rows = [...document.querySelectorAll(".wall .hook")].map((e) => e.getBoundingClientRect().bottom).filter((b) => b < h);
    return Math.ceil(Math.max(...rows)) + 28;
  });
  await p.screenshot({ path: join(OUT, name), clip: { x: 0, y: 0, width: 1280, height: bottom } });
}

// The hero: a photo dropped on the wall, which narrows it to what can take a photo.
const wall = await page(browser, "light");
await wall.evaluate(() => window.__drop(["/Users/sam/Pictures/Harbour at dusk.jpg"]));
await wall.waitForTimeout(1200);
await shootWall(wall, "wall.png");

const bench = await page(browser, "light");
await bench.getByRole("button", { name: /Compress video/ }).click();
await bench.evaluate((home) => window.__drop([`${home}/Lisbon trip.mov`, `${home}/Team offsite.mp4`, `${home}/Launch demo.mov`]), HOME);
await bench.getByRole("button", { name: "Run", exact: true }).click();
await bench.waitForSelector(".files li.done");
await bench.waitForSelector(".bar:not(.indet)");
await bench.waitForTimeout(400);
await bench.screenshot({ path: join(OUT, "bench.png") });

const chains = await page(browser, "dark");
await chains.getByRole("button", { name: /Chains/ }).click();
await chains.fill("#chain-name", "Shrink & stamp");
for (const id of ["pdf-compress", "pdf-stamp"]) {
  await chains.selectOption("#chain-add", id);
  await chains.click("#chain-add-btn");
}
await chains.waitForTimeout(1000);
await chains.screenshot({ path: join(OUT, "chains.png") });

await browser.close();
console.log("docs/wall.png docs/bench.png docs/chains.png");
