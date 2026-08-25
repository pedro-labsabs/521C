#!/usr/bin/env node
// Documentation structure check (issue #15).
//
// Verifies that every top-level document under docs/ declares its role with an
// "**Authority:**" or "**Role:**" marker near the top, and that the docs index
// (docs/README.md) exists. Deliberately lightweight: this is a review aid, not
// a markdown linter.

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";

const docsDir = new URL("../docs/", import.meta.url).pathname;
const MARKER = /^\*\*(Authority|Role):\*\*/m;
const HEAD_LINES = 12;

const failures = [];

if (!existsSync(join(docsDir, "README.md"))) {
  failures.push("docs/README.md (documentation index) is missing");
}

const files = readdirSync(docsDir).filter(
  (f) => f.endsWith(".md") && f !== "README.md",
);

for (const file of files) {
  const text = readFileSync(join(docsDir, file), "utf8");
  const head = text.split("\n").slice(0, HEAD_LINES).join("\n");
  if (!MARKER.test(head)) {
    failures.push(
      `docs/${file}: missing "**Authority:**" or "**Role:**" marker in the first ${HEAD_LINES} lines`,
    );
  }
}

if (failures.length > 0) {
  console.error("docs check failed:");
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log(`✔ docs check passed (${files.length} top-level documents + index)`);
