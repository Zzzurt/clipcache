// Renders the PRE-redesign UI for side-by-side comparison.
//
// Both halves are pulled from git rather than from memory: `legacy-index.html`
// and `legacy-styles.css` are `git show HEAD:src/...` snapshots, so the "before"
// image stays an honest record even after src/ moves on.
//
// Usage: node build-before.mjs      → mock/before.html
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const mock = (p) => resolve(HERE, "mock", p);

let html = readFileSync(mock("legacy-index.html"), "utf8");

// The snapshot references the stylesheet next to it; point at the frozen copy.
html = html.replace(
  '<link rel="stylesheet" href="styles.css" />',
  '<link rel="stylesheet" href="legacy-styles.css" />'
);

// The old page loaded /main.js and talked to Tauri, which is absent here. Swap
// in a small script that fills the same DOM the way main.js did.
html = html.replace(
  '<script type="module" src="/main.js" defer></script>',
  `<script type="module">${readFileSync(mock("before-fixtures.mjs"), "utf8")}</script>`
);

writeFileSync(mock("before.html"), html);
console.log("wrote design/mock/before.html");
