// Checks that every relative link and image reference in the tracked markdown
// files resolves on disk. Run from the repository root: node scripts/linkcheck.mjs
//
// design/README.md 不入库（本地设计工作区），但那份文档里的相对引用同样会是
// 死链，所以磁盘上存在时一并检查；不在时静默跳过，不把「本地没写设计稿」当成失败。
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const files = ["README.md", "design/README.md"].filter((f) => existsSync(f));
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

if (!files.length) {
  console.error("没有找到可检查的 markdown，检查从仓库根目录运行");
  process.exitCode = 1;
} else {
  process.exitCode = bad ? 1 : 0;
}
