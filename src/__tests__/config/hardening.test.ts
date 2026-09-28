import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = process.cwd();

function readJson(rel: string) {
  return JSON.parse(fs.readFileSync(path.join(ROOT, rel), "utf8"));
}

describe("hardening config (SPEC-001 §5 DG-7)", () => {
  it("CSP is enabled and allows Tauri IPC", () => {
    const conf = readJson("src-tauri/tauri.conf.json");
    const csp = conf.app.security.csp;
    expect(typeof csp).toBe("string");
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain("ipc:");
    expect(csp).toContain("http://ipc.localhost");
    // script-src must not allow inline scripts (that would nullify the CSP).
    expect(csp).toContain("script-src 'self';");
    expect(csp).not.toMatch(/script-src[^;]*unsafe-inline/);
  });

  it("capabilities are minimized", () => {
    const caps = readJson("src-tauri/capabilities/default.json");
    const perms: string[] = caps.permissions;
    for (const removed of [
      "shell:allow-execute",
      "shell:allow-spawn",
      "shell:allow-stdin-write",
      "shell:allow-kill",
      "shell:allow-open",
      "http:default",
    ]) {
      expect(perms).not.toContain(removed);
    }
  });
});
