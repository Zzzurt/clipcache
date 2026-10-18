import { readFileSync } from "node:fs";
const h = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const js = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const ids = [...h.matchAll(/id="([^"]+)"/g)].map((m) => m[1]);
const used = [...js.matchAll(/\$\("([^"]+)"\)/g)].map((m) => m[1]);
const missing = [...new Set(used)].filter((u) => !ids.includes(u));
console.log("ids:", ids.length, "| used:", new Set(used).size);
console.log("MISSING:", missing.length ? missing : "none");
const dup = ids.filter((v, i) => ids.indexOf(v) !== i);
console.log("DUP:", dup.length ? dup : "none");
// classes main.js creates must exist in the stylesheet
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const cls = new Set([...js.matchAll(/className\s*=\s*"([^"]+)"/g)].flatMap((m) => m[1].split(/\s+/)));
const absent = [...cls].filter((c) => c && !c.includes("$") && !css.includes("." + c));
console.log("classes set by JS but unstyled:", absent.length ? absent : "none");
