// Renders a local HTML file to PNG with headless Chrome.
// usage: node shot.mjs <page> <out.png> [width] [height] [scale]
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { mkdirSync } from "node:fs";

const here = dirname(fileURLToPath(import.meta.url));
const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";

const [, , page, out, w = "1000", h = "700", scale = "2"] = process.argv;
if (!page || !out) {
  console.error("usage: node shot.mjs <page.html> <out.png> [w] [h] [scale]");
  process.exit(1);
}

const pageAbs = resolve(here, page);
const outAbs = resolve(here, out);
mkdirSync(dirname(outAbs), { recursive: true });

const args = [
  "--headless=new",
  "--disable-gpu",
  "--hide-scrollbars",
  "--allow-file-access-from-files",
  `--user-data-dir=${join(here, ".chrome-profile")}`,
  `--force-device-scale-factor=${scale}`,
  `--window-size=${w},${h}`,
  "--virtual-time-budget=2500",
  `--screenshot=${outAbs}`,
  "file:///" + pageAbs.replace(/\\/g, "/"),
];

const r = spawnSync(CHROME, args, { encoding: "utf8" });
const line = (r.stdout || "") + (r.stderr || "");
const wrote = line.match(/written to file .*/);
console.log(wrote ? wrote[0] : line.slice(-400));
