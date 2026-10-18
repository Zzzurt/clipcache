// Renders the PRE-redesign UI for side-by-side comparison.
//
// The "before" half is pulled straight out of git history rather than kept as a
// checked-in copy: FROZEN_COMMIT is the last commit before the redesign landed,
// so the comparison stays an honest record without duplicating files that the
// repository already stores. Bump it only if the baseline genuinely moves.
//
// Usage: node build-before.mjs      → mock/before.html
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const mock = (p) => resolve(HERE, "mock", p);

// Parent of the commit that rewrote the interface.
const FROZEN_COMMIT = "384f250";

const gitShow = (path) =>
  execFileSync("git", ["show", `${FROZEN_COMMIT}:${path}`], {
    cwd: HERE,
    encoding: "utf8",
    maxBuffer: 1 << 24,
  });

let html = gitShow("src/index.html");

// The snapshot expects the stylesheet beside it and a Tauri runtime that is not
// here. Point the link at a generated copy and swap the module script for
// fixtures that fill the same DOM the old main.js did.
const css = gitShow("src/styles.css");
writeFileSync(mock("legacy-styles.css"), css);

html = html.replace(
  '<link rel="stylesheet" href="styles.css" />',
  '<link rel="stylesheet" href="legacy-styles.css" />'
);
html = html.replace(
  '<script type="module" src="/main.js" defer></script>',
  `<script type="module">${readFileSync(mock("before-fixtures.mjs"), "utf8")}</script>`
);

writeFileSync(mock("before.html"), html);
console.log(`wrote design/mock/before.html from ${FROZEN_COMMIT}`);
