import { readFileSync, existsSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Local-first network guard (issue #12).
//
// 521C is local-first / no-telemetry: the default shell must not make implicit
// third-party runtime requests, and the dev server must bind to loopback unless
// the user explicitly opts in to LAN exposure. This test pins that contract at
// the source level so it runs in CI via `npm test`. The companion build audit
// (`npm run audit:network`) additionally scans the compiled `dist/` output.

const URL_RE = /https?:\/\/[a-zA-Z0-9._/-]+/g;

// Same intentional exceptions as scripts/audit-network.mjs — keep in sync.
const ALLOWLIST = [
  "http://www.w3.org/", // XML/SVG namespace identifiers, never fetched
  "https://react.dev/errors/", // React dev error-message links, never fetched
  "https://tailwindcss.com", // Tailwind license banner comment, never fetched
];

function isLocalOrReservedHost(url: string): boolean {
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

function thirdPartyUrls(text: string): string[] {
  const out: string[] = [];
  for (const match of text.match(URL_RE) ?? []) {
    if (!ALLOWLIST.some((p) => match.startsWith(p)) && !isLocalOrReservedHost(match)) {
      out.push(match);
    }
  }
  return out;
}

describe("local-first network contract (issue #12)", () => {
  it("default shell has no third-party runtime URLs (fonts/CDN/analytics)", () => {
    const shell = [
      readFileSync("src/routes/__root.tsx", "utf8"),
      readFileSync("src/styles.css", "utf8"),
    ].join("\n");
    expect(thirdPartyUrls(shell)).toEqual([]);
  });

  it("does not preconnect or load remote stylesheets in the root route", () => {
    const root = readFileSync("src/routes/__root.tsx", "utf8");
    expect(root).not.toMatch(/preconnect/i);
    expect(root).not.toMatch(/fonts\.googleapis\.com/);
    expect(root).not.toMatch(/fonts\.gstatic\.com/);
  });

  it("dev server binds to loopback by default", () => {
    const vite = readFileSync("vite.config.ts", "utf8");
    // The dev server block must not expose all interfaces.
    expect(vite).not.toMatch(/server:\s*{[^}]*host:\s*"0\.0\.0\.0"/s);
    const pkg = JSON.parse(readFileSync("package.json", "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts.dev).not.toContain("0.0.0.0");
  });

  it("provides an explicit opt-in for LAN development", () => {
    const pkg = JSON.parse(readFileSync("package.json", "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts["dev:lan"]).toBeDefined();
    expect(pkg.scripts["dev:lan"]).toContain("0.0.0.0");
  });

  it("ships a build audit that guards the compiled output", () => {
    expect(existsSync("scripts/audit-network.mjs")).toBe(true);
    const pkg = JSON.parse(readFileSync("package.json", "utf8")) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts["audit:network"]).toContain("audit-network.mjs");
  });
});
