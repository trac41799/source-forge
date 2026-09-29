/**
 * IPC contract test (SPEC-001 §3.1, Step 2.1).
 *
 * Parses the Tauri handler registry from `src-tauri/src/lib.rs` and every
 * `invoke()` call site in `src/` (resolving `IPC.*` constants through
 * `src/lib/ipc/commands.ts`) and asserts:
 *
 *   1. no invoked command is missing from the registry (no dead invokes)
 *   2. every `IPC.*` key used at a call site is defined in commands.ts
 *   3. inline `invoke("...")` literals are a ratchet — no new sites allowed
 *      beyond `ipc-literal-baseline.json`
 *   4. the registered-but-unused set matches `ipc-unused.snapshot.json`
 *
 * Regenerate the baseline/snapshot deliberately with:
 *   UPDATE_IPC_SNAPSHOT=1 npx vitest run src/__tests__/contracts/ipc-contract.test.ts
 */
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// vitest runs with cwd = project root (main repo or a worktree of it).
const ROOT = process.cwd();
const BASELINE_PATH = path.join(
  ROOT,
  "src/__tests__/contracts/ipc-literal-baseline.json"
);
const SNAPSHOT_PATH = path.join(
  ROOT,
  "src/__tests__/contracts/ipc-unused.snapshot.json"
);
const UPDATE = process.env.UPDATE_IPC_SNAPSHOT === "1";

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(p));
    else if (/\.(ts|tsx)$/.test(entry.name)) out.push(p);
  }
  return out;
}

/** Command names registered in the Tauri `generate_handler![...]` block. */
function registeredCommands(): string[] {
  const lib = fs.readFileSync(
    path.join(ROOT, "src-tauri/src/lib.rs"),
    "utf8"
  );
  const block = lib.match(/generate_handler!\[([\s\S]*?)\]/);
  if (!block) throw new Error("generate_handler! block not found in lib.rs");

  const names: string[] = [];
  for (const rawLine of block[1].split(/\r?\n/)) {
    const line = rawLine.trim().replace(/,$/, "");
    const m = line.match(/^(?:[A-Za-z_]\w*::)?([A-Za-z_]\w*)$/);
    if (m) names.push(m[1]);
  }
  return [...new Set(names)].sort();
}

/** `IPC` constant map from src/lib/ipc/commands.ts. */
function ipcConstants(): Record<string, string> {
  const file = path.join(ROOT, "src/lib/ipc/commands.ts");
  if (!fs.existsSync(file)) return {};
  const src = fs.readFileSync(file, "utf8");
  const map: Record<string, string> = {};
  for (const m of src.matchAll(
    /^\s*([A-Za-z_]\w*)\s*:\s*"([^"]+)"\s*,?/gm
  )) {
    map[m[1]] = m[2];
  }
  return map;
}

function scanInvocations() {
  const constants = ipcConstants();
  const files = walk(path.join(ROOT, "src")).filter(
    (f) => !f.includes("__tests__")
  );

  const invoked = new Set<string>();
  const literalSites: string[] = [];
  const unknownKeys: string[] = [];
  const siteCounts = new Map<string, number>();

  for (const file of files) {
    const rel = path.relative(ROOT, file).split(path.sep).join("/");
    const src = fs.readFileSync(file, "utf8");

    // Match single quotes, double quotes and backticks — the previous
    // double-quote-only regex silently skipped a large share of call sites.
    for (const m of src.matchAll(/\binvoke\b[^(]*\(\s*['"`]([^'"`]+)['"`]/g)) {
      invoked.add(m[1]);
      const key = `${rel}::${m[1]}`;
      const occurrence = (siteCounts.get(key) ?? 0) + 1;
      siteCounts.set(key, occurrence);
      literalSites.push(`${key}#${occurrence}`);
    }
    for (const m of src.matchAll(/\binvoke\b[^(]*\(\s*IPC\.([A-Za-z_]\w*)/g)) {
      const name = constants[m[1]];
      if (name) invoked.add(name);
      else unknownKeys.push(`${rel}::IPC.${m[1]}`);
    }
  }

  return {
    invoked: [...invoked].sort(),
    literalSites: literalSites.sort(),
    unknownKeys,
  };
}

function readJson<T>(p: string, fallback: T): T {
  try {
    return JSON.parse(fs.readFileSync(p, "utf8")) as T;
  } catch {
    return fallback;
  }
}

function writeJson(p: string, value: unknown) {
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, JSON.stringify(value, null, 2) + "\n", "utf8");
}

const registered = registeredCommands();
const { invoked, literalSites, unknownKeys } = scanInvocations();

describe("IPC contract", () => {
  it("every invoked command is registered (no dead invokes)", () => {
    const missing = invoked.filter((name) => !registered.includes(name));
    expect(missing, "dead invokes (frontend → missing backend command)").toEqual(
      []
    );
  });

  it("every IPC.* key resolves to a command name", () => {
    expect(unknownKeys).toEqual([]);
  });

  it("scans single-quoted invokes too (regression)", () => {
    // Regression: the scanner previously only matched double quotes, hiding
    // dead commands such as `detect_stack` / `check_skillbridge_status`.
    expect(
      literalSites.some((site) => site.startsWith("src/stores/agentStore.ts::"))
    ).toBe(true);
  });

  it("no new inline invoke(\"...\") literals beyond the baseline (ratchet)", () => {
    const baseline = new Set(readJson<string[]>(BASELINE_PATH, []));
    const added = literalSites.filter((site) => !baseline.has(site));
    if (UPDATE) {
      writeJson(BASELINE_PATH, literalSites);
      return;
    }
    expect(
      added,
      "new inline invoke() literals — use IPC constants from src/lib/ipc/commands.ts"
    ).toEqual([]);
  });

  it("registered-but-unused commands match the snapshot", () => {
    const unused = registered.filter((name) => !invoked.includes(name));
    if (UPDATE) {
      writeJson(SNAPSHOT_PATH, unused);
      return;
    }
    expect(unused, "unused command set changed — update the snapshot deliberately (UPDATE_IPC_SNAPSHOT=1)").toEqual(
      readJson<string[]>(SNAPSHOT_PATH, [])
    );
  });
});
