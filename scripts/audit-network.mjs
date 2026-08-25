#!/usr/bin/env node
// Network audit (issue #12): fail if the default shell or the built application
// contains implicit third-party runtime URLs.
//
// 521C is local-first / no-telemetry. The app must not fetch fonts, CSS, JS,
// analytics or any other resource from a third party at runtime. User-initiated
// navigation to documentation/source links is allowed by the privacy contract,
// but no such link is bundled into the runtime today; any future one must be
// added to ALLOWLIST below with a justification.
//
// Usage: node scripts/audit-network.mjs   (run after `npm run build` for a full audit)

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;

// URL prefixes that are NOT runtime network requests and are therefore allowed.
// Each entry is an intentional, documented exception (issue #12). Anything else
// fails the audit so implicit third-party runtime traffic cannot reappear.
const ALLOWLIST = [
  // XML / SVG namespace identifiers — never fetched.
  "http://www.w3.org/",
  // React error-message documentation links (embedded in dev error text, never fetched).
  "https://react.dev/errors/",
  // Tailwind CSS license/attribution banner comment in generated CSS — not a request.
  "https://tailwindcss.com",
];

// Hosts that are inherently local or reserved for documentation, never third-party.
function isLocalOrReservedHost(url) {
  try {
    const host = new URL(url).hostname;
    return (
      host === "localhost" ||
      host === "127.0.0.1" ||
      host === "::1" ||
      host === "example.com" ||
      host.endsWith(".example.com")
    );
  } catch {
    return false;
  }
}

// Source files that form the default shell (always audited, even without a build).
const SHELL_SOURCES = ["src/routes/__root.tsx", "src/styles.css"];

const URL_RE = /https?:\/\/[a-zA-Z0-9._/-]+/g;

function isAllowed(url) {
  return (
    ALLOWLIST.some((prefix) => url.startsWith(prefix)) || isLocalOrReservedHost(url)
  );
}

function collectFiles(dir, exts, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      collectFiles(full, exts, out);
    } else if (exts.some((e) => full.endsWith(e))) {
      out.push(full);
    }
  }
  return out;
}

function scanFile(file, findings) {
  const text = readFileSync(file, "utf8");
  for (const match of text.match(URL_RE) ?? []) {
    if (!isAllowed(match)) {
      findings.push({ file: relative(ROOT, file), url: match });
    }
  }
}

const findings = [];

// Always audit the shell sources.
for (const rel of SHELL_SOURCES) {
  const full = join(ROOT, rel);
  if (existsSync(full)) scanFile(full, findings);
}

// Audit the built output when present.
const dist = join(ROOT, "dist");
let auditedDist = false;
if (existsSync(dist)) {
  auditedDist = true;
  for (const file of collectFiles(dist, [".js", ".css", ".html"])) {
    scanFile(file, findings);
  }
}

if (findings.length > 0) {
  console.error("\u2716 network audit FAILED — implicit third-party runtime URLs found:");
  for (const f of findings) {
    console.error(`  ${f.file}: ${f.url}`);
  }
  console.error(
    "\nRemove these, or add an explicit, justified exception to ALLOWLIST in scripts/audit-network.mjs.",
  );
  process.exit(1);
}

console.log(
  `\u2714 network audit passed${auditedDist ? " (shell sources + dist/)" : " (shell sources only; run `npm run build` first for a full audit)"}`,
);
