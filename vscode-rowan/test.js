// node test.js — checks the hover rendering against a real --facts document.
"use strict";
const assert = require("assert");
const { execFileSync } = require("child_process");
const path = require("path");
const { renderHover, indexResult } = require("./extension.js");

const root = path.join(__dirname, "..");
const bin = process.env.ROWAN_BIN || path.join(root, "rowan", "target", "release", "rowan");
const file = path.join(root, "tests", "cases", "a01_conditional_key.py");
const res = JSON.parse(execFileSync(bin, [file, "--facts"], { maxBuffer: 64 * 1024 * 1024 }).toString());
const idx = indexResult(res);

const r8 = renderHover("r", idx.facts.get("8:r"), idx.verdicts.get(8) || []);
assert(r8.includes('- value: `{"id": ..., "role"?: "admin"}`'), r8);
assert(r8.includes("`?` set on some paths only"), r8);

const r9 = renderHover("r", idx.facts.get("9:r"), (idx.verdicts.get(9) || []).filter((v) => v.text.includes("r")));
assert(r9.includes("⚠ `r[\"role\"]`"), r9);

const u = renderHover("u", idx.facts.get("8:u"), []);
assert(u.includes("requires of name u: always 'id'"), u);

const d = renderHover("d", idx.facts.get("2:d"), []);
assert(d.includes("relative to its entry") || d.includes("object"), d);

assert.strictEqual(renderHover("zzz", [], []), null);
console.log("ok");
console.log("\n--- sample hover for r at line 8 ---\n" + r8);
console.log("\n--- sample hover for d inside build (line 2) ---\n" + d);
