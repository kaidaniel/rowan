// node test.js — checks the hover rendering against a real --facts document.
"use strict";
const assert = require("assert");
const { execFileSync } = require("child_process");
const path = require("path");
const { renderHover, indexResult, prettyValue } = require("./extension.js");

const root = path.join(__dirname, "..");
const bin = process.env.ROWAN_BIN || path.join(root, "rowan", "target", "release", "rowan");
const file = path.join(root, "tests", "cases", "a01_conditional_key.py");
const res = JSON.parse(execFileSync(bin, [file, "--facts"], { maxBuffer: 64 * 1024 * 1024 }).toString());
const idx = indexResult(res);

const r8 = renderHover("r", idx.facts.get("8:r"), idx.verdicts.get(8) || []);
assert(r8.includes('- value: `{"id": ?, "role"?: "admin"}`'), r8);
assert(r8.includes('`"k"?:` set on some paths only'), r8);

const r9 = renderHover("r", idx.facts.get("9:r"), (idx.verdicts.get(9) || []).filter((v) => v.text.includes("r")));
assert(r9.includes("⚠ `r[\"role\"]`"), r9);

const u = renderHover("u", idx.facts.get("8:u"), []);
assert(u.includes("requires of name u: always 'id'"), u);

const d = renderHover("d", idx.facts.get("2:d"), []);
assert(d.includes("relative to its entry") || d.includes("object"), d);

assert.strictEqual(renderHover("zzz", [], []), null);

// long values break over lines of at most 40 columns, indented by level; small ones stay flat
const repo = 'Repo(events=[{"key": 1, "op": "add"}, {"key": 2, "op": "add"}, {"key": 3, "op": "add"}], index_fns={"tier": build_world.lambda@L43}, indexes={"tier": {"basic": […], "gold": […], "silver": […]}}, items={1: Customer(address={…}, cid=1, name="Ann", orders=[…], tier="gold"), 2: Customer(address={…}, cid=2, name="Bob", orders=[…], tier="basic"), 3: Customer(address={…}, cid=3, name="Cy", orders=[…], tier="silver")}, key_fn=build_world.lambda@L43, name="customers")';
const lines = prettyValue(repo);
assert(lines.length > 10, lines.join("\n"));
for (const l of lines) assert(l.length <= 40, `too wide: ${JSON.stringify(l)}`);
assert.strictEqual(lines[0], "Repo(");
assert(lines.includes('  name="customers"'), lines.join("\n"));
assert(lines.includes('    {"key": 1, "op": "add"},'), lines.join("\n"));
assert.deepStrictEqual(prettyValue("[1, 2, 3]"), ["[1, 2, 3]"]);
assert.strictEqual(lines.join("").replace(/\s+/g, ""), repo.replace(/\s+/g, ""));
// `--value-depth N` (the `rowan.valueDepth` setting) cuts nested containers at depth N
const nested = path.join(root, "tests", "cases", "q118_recursive_nesting.py");
const textsAt = (n) => JSON.parse(execFileSync(bin, [nested, "--facts", "--value-depth", String(n)], { maxBuffer: 64 * 1024 * 1024 }).toString()).facts.map((f) => f.text || "").join("\n");
const shallow = textsAt(1);
const deep = textsAt(3);
assert(shallow.length < deep.length, "depth 1 should render less than depth 3");
assert(/[\[{(]…[\]})]/.test(shallow), shallow.slice(0, 400));
// an unknown value: where it comes from, no key-set talk
const z = path.join(root, "tests", "cases", "z419_manually_created.py");
const zi = indexResult(JSON.parse(execFileSync(bin, [z, "--facts"], { maxBuffer: 64 * 1024 * 1024 }).toString()));
const x1 = renderHover("x1", zi.facts.get("6:x1"), []);
assert(x1.includes("- value: `?`") && x1.includes("whatever a caller passes as `b` to `f`"), x1);
const u8 = renderHover("_", zi.facts.get("8:_"), []);
assert(u8.includes("is the result of `a(b)(a)` (line 8)") && !u8.includes("may also be"), u8);
assert(!x1.includes("**?") && !x1.includes("keys possible"), x1);
console.log("\n--- sample long value ---\n" + lines.join("\n"));
console.log("ok");
console.log("\n--- sample hover for r at line 8 ---\n" + r8);
console.log("\n--- sample hover for d inside build (line 2) ---\n" + d);
