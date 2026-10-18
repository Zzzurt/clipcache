// Checks that every relative link and image reference in the tracked markdown
// files resolves on disk. Run from the repository root: node design/linkcheck.mjs
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const files = ["README.md", "design/README.md"];
let bad = 0;

for (const file of files) {
  const dir = dirname(file);
  const text = readFileSync(file, "utf8");
  const targets = [
    ...[...text.matchAll(/!?\[[^\]]*\]\(([^)\s]+)\)/g)].map((m) => m[1]),
    ...[...text.matchAll(/<img[^>]+src="([^"]+)"/g)].map((m) => m[1]),
  ].filter((u) => !/^(https?:|#|mailto:)/.test(u));

  const missing = targets.filter((u) => !existsSync(resolve(dir, u.split("#")[0])));
  console.log(`${file}: ${targets.length} 个相对引用，${missing.length ? "失效 → " + missing.join(", ") : "全部有效"}`);
  bad += missing.length;
}

process.exitCode = bad ? 1 : 0;
