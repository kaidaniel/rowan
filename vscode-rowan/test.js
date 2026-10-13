// node test.js — checks the hover and diagnostic rendering against real --facts documents:
// what particular hovers say, and that no hover on several real programs uses internal
// names or a mark it does not explain.
"use strict";
const assert = require("assert");
const fs = require("fs");
const os = require("os");
const { execFileSync } = require("child_process");
const path = require("path");
const { renderHover, indexResult, prettyValue, verdictsForWord, severityFor, diagnosticMessage, shorten } = require("./extension.js");

const root = path.join(__dirname, "..");
const bin = process.env.ROWAN_BIN || path.join(root, "rowan", "target", "release", "rowan");
const facts = (file, ...args) => JSON.parse(execFileSync(bin, [file, "--facts", ...args], { maxBuffer: 64 * 1024 * 1024 }).toString());

/** Every hover of a file, as the editor shows it (the word at its first place on the line). */
function allHovers(file) {
  const res = facts(file);
  const src = fs.readFileSync(file, "utf8").split("\n");
  const idx = indexResult(res);
  const out = [];
  for (const [key, fs_] of idx.facts) {
    const [line, name] = [Number(key.split(":")[0]), key.split(":").slice(1).join(":")];
    const m = new RegExp(`(^|[^A-Za-z0-9_])(${name})($|[^A-Za-z0-9_])`).exec(src[line - 1] || "");
    const start = m ? m.index + m[1].length : 0;
    const h = renderHover(name, fs_, verdictsForWord(idx.verdicts.get(line) || [], name, start, start + name.length));
    if (h) out.push({ line, name, h });
  }
  return { res, hovers: out };
}

/** The hover of `name` on `line` of an indexed document. */
function hover(idx, src, line, name) {
  const m = new RegExp(`(^|[^A-Za-z0-9_])(${name})($|[^A-Za-z0-9_])`).exec(src[line - 1] || "");
  const start = m ? m.index + m[1].length : 0;
  return renderHover(name, idx.facts.get(`${line}:${name}`) || [], verdictsForWord(idx.verdicts.get(line) || [], name, start, start + name.length));
}

// ---- a dict with a key set on some paths, a typo, an unknown argument
const a01 = path.join(root, "tests", "cases", "a01_conditional_key.py");
const a01src = fs.readFileSync(a01, "utf8").split("\n");
const a01idx = indexResult(facts(a01));
const r8 = hover(a01idx, a01src, 8, "r");
assert(r8.includes('```python\n{"id": u.id, "role"?: "admin"}\n```'), r8);
assert(r8.includes('`"k"?:` set on some paths only'), r8);
assert(r8.includes("- created on line 2"), r8);
const r9 = hover(a01idx, a01src, 9, "r");
assert(r9.includes('- ⚠ `r["role"]`: key "role" is only set on some paths'), r9);
assert(r9.includes("⚠ fails on some paths"), r9);
const r13 = hover(a01idx, a01src, 13, "r");
assert(r13.includes('- ✗ `r["rolle"]`: key "rolle" is never set'), r13);
const u8 = hover(a01idx, a01src, 8, "u");
assert(u8.includes("- the code reads `u.id`"), u8);
assert(u8.includes("not defined in this file"), u8);
// a parameter: what callers pass (`u`), what the function reads of it
const user2 = hover(a01idx, a01src, 2, "user");
assert(user2.includes("```python\nu\n```") && user2.includes("- the code reads `user.id`"), user2);
assert.strictEqual(renderHover("zzz", [], []), null);
// an alternative too wide for its line keeps whole items (`…` for the rest), not a cut word
assert.strictEqual(shorten('{"id": 1, "owner": "bo", "priority": 2, "tags": [], "title": "printer jam"}', 60), '{"id": 1, "owner": "bo", "priority": 2, "tags": [], …}');
assert.strictEqual(shorten('Ticket(id=1, title="a very long title that goes on and on and on")', 30), "Ticket(id=1, …)");
assert.strictEqual(shorten('x' + "y".repeat(70), 10), "xyyyyyyyy…");

// ---- a call rowan no longer follows (past its function's budget): said where it changes a value
{
  const z440 = path.join(root, "tests", "cases", "z440_past_budget_changes_globals.py");
  const res = JSON.parse(execFileSync(bin, [z440, "--facts"], { env: { ...process.env, ROWAN_LIMITS: "budget=2" }, maxBuffer: 64 * 1024 * 1024 }).toString());
  const h = hover(indexResult(res), fs.readFileSync(z440, "utf8").split("\n"), 58, "STATE");
  assert(h.includes("- may have been changed by `drop()` (line 57, a call rowan no longer follows)"), h);
}

// ---- diagnostics: only accesses that may fail, in words, with their call path
assert.strictEqual(severityFor("missing"), "error");
assert.strictEqual(severityFor("possibly"), "warning");
assert.strictEqual(severityFor("input"), null);
assert.strictEqual(severityFor("likely"), null);
const v13 = (a01idx.verdicts.get(13) || [])[0];
assert.strictEqual(diagnosticMessage(v13), 'key "rolle" is never set');
assert.strictEqual(diagnosticMessage({ message: "m", chain: [5, 9] }), "m (via L5→L9)");

// ---- long values break over lines of at most 40 (here) columns, indented by level;
// small ones, and calls with one argument, stay whole
const repo = 'Repo(events=[{"key": 1, "op": "add"}, {"key": 2, "op": "add"}, {"key": 3, "op": "add"}], index_fns={"tier": <lambda on line 43>}, indexes={"tier": {"basic": […], "gold": […], "silver": […]}}, items={1: Customer(address={…}, cid=1, name="Ann", orders=[…], tier="gold"), 2: Customer(address={…}, cid=2, name="Bob", orders=[…], tier="basic"), 3: Customer(address={…}, cid=3, name="Cy", orders=[…], tier="silver")}, name="customers")';
const lines = prettyValue(repo, 40);
assert(lines.length > 10, lines.join("\n"));
for (const l of lines) assert(l.length <= 40, `too wide: ${JSON.stringify(l)}`);
assert.strictEqual(lines[0], "Repo(");
assert(lines.includes('  name="customers"'), lines.join("\n"));
assert(lines.includes('    {"key": 1, "op": "add"},'), lines.join("\n"));
assert.deepStrictEqual(prettyValue("[1, 2, 3]"), ["[1, 2, 3]"]);
assert.strictEqual(lines.join("").replace(/\s+/g, ""), repo.replace(/\s+/g, ""));
assert.deepStrictEqual(prettyValue('{"t": 10.0 | json.loads(os.environ["APP_CONFIG"])["timeout"], **?}', 40), ["{", '  "t": 10.0 | json.loads(os.environ["APP_CONFIG"])["timeout"],', "  **?", "}"]);
// one of several long values: one per line
const alt = prettyValue('OrderRepo(orders={}) | OrderRepo(_seq=0, by_customer=Counter({}), lines={}, orders={})', 50);
assert.strictEqual(alt[0], "  OrderRepo(orders={})", alt.join("\n"));
assert(alt[1].startsWith("| OrderRepo("), alt.join("\n"));
// `--value-depth N` (the `rowan.valueDepth` setting) cuts nested containers at depth N
const nested = path.join(root, "tests", "cases", "q118_recursive_nesting.py");
const textsAt = (n) => facts(nested, "--value-depth", String(n)).facts.map((f) => f.text || "").join("\n");
const shallow = textsAt(1);
const deep = textsAt(3);
assert(shallow.length < deep.length, "depth 1 should render less than depth 3");
assert(/[\[{(]…[\]})]/.test(shallow), shallow.slice(0, 400));

// ---- unknowns named by where they come from; nothing said about what is not known
const z = path.join(root, "tests", "cases", "z419_manually_created.py");
const zsrc = fs.readFileSync(z, "utf8").split("\n");
const zi = indexResult(facts(z));
const x1 = hover(zi, zsrc, 6, "x1");
assert(x1.includes("```python\nb\n```"), x1);
assert(!x1.includes("**?") && !x1.includes("unknown"), x1);
const u8z = hover(zi, zsrc, 8, "_");
assert(u8z.includes("```python\na(b)(a)\n```"), u8z);

// ---- tensors (nanoGPT's attention): the shape in the function's own terms, dims named by
// the locals that hold them, the line's requirement, what calls gave
const g = path.join(root, "tests", "torch", "models", "m02_nanogpt.py");
const gsrc = fs.readFileSync(g, "utf8").split("\n");
const gi = indexResult(facts(g));
const qkv = hover(gi, gsrc, 64, "qkv");
assert(qkv.includes("```python\nTensor[B, T, 3*self.n_embd]\n```"), qkv);
assert(qkv.includes("- `B` = `x.shape[0]`") && qkv.includes("- needs `C == self.n_embd`"), qkv);
assert(qkv.includes("- in calls: `Tensor[3, 10, 96]`"), qkv);
const gx = hover(gi, gsrc, 64, "x");
assert(gx.includes("Tensor[B, T, C]") && gx.includes("- `B, T, C` = `x.shape`"), gx);

// ---- model objects by their type, not as `{}`; names of definitions get no hover
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rowan-test-"));
const kinds = path.join(tmp, "kinds.py");
fs.writeFileSync(kinds, [
  "import re, argparse",
  "from types import SimpleNamespace",
  "m = re.match(r'a(b)', 'ab')",
  "p = argparse.ArgumentParser()",
  "p.add_argument('--n', type=int, default=3)",
  "args = p.parse_args([])",
  "ns = SimpleNamespace(a=1)",
  "def gen():",
  "    yield 1",
  "g = gen()",
  "t = 'line1\\nline2'",
  "s = 'é' + 'x'; d = {'k': 1}; v = d['q']",
  "",
].join("\n"));
const ksrc = fs.readFileSync(kinds, "utf8").split("\n");
const ki = indexResult(facts(kinds));
assert(hover(ki, ksrc, 3, "m").includes("<Match>"), hover(ki, ksrc, 3, "m"));
assert(hover(ki, ksrc, 6, "args").includes("Namespace(n=3)"), hover(ki, ksrc, 6, "args"));
assert(hover(ki, ksrc, 7, "ns").includes("namespace(a=1)"), hover(ki, ksrc, 7, "ns"));
assert(hover(ki, ksrc, 10, "g").includes("<generator gen>"), hover(ki, ksrc, 10, "g"));
assert(hover(ki, ksrc, 11, "t").includes('"line1\\nline2"'), hover(ki, ksrc, 11, "t"));
assert.strictEqual(hover(ki, ksrc, 4, "argparse"), null);
// columns in UTF-16 units: the access after a non-ASCII character is where the editor has it
const vq = (ki.verdicts.get(12) || []).find((v) => v.text === "d['q']");
assert(vq && vq.col === ksrc[11].indexOf("d['q']"), JSON.stringify(vq));
const dq = ksrc[11].indexOf("d['q']");
const hd = renderHover("d", ki.facts.get("12:d"), verdictsForWord(ki.verdicts.get(12), "d", dq, dq + 1));
assert(hd.includes("- ✗ `d['q']`: key \"q\" is never set"), hd);
// (the other `d` on the line, being assigned, has no access)
assert(!hover(ki, ksrc, 12, "d").includes("✗"), hover(ki, ksrc, 12, "d"));

// ---- every hover of several real programs: no internal names, every mark explained
const files = [
  a01, z, g, kinds,
  path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "shop", "repo.py"),
  path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "b001_shop.py"),
  path.join(root, "tests", "apps", "large", "c001_platform_pkg", "dp", "pipeline.py"),
  path.join(root, "tests", "pandas", "models", fs.readdirSync(path.join(root, "tests", "pandas", "models")).filter((f) => f.endsWith(".py"))[0]),
];
const INTERNAL = [/PH@/, /\bM\d+:L\d+/, /#\d{3,}/, /\bparam [\w.]+\.\w/, /\b(item|attr)\[/, /`\?`: unknown/, /\bdef \w/, /\bresult of [^`]*@L/, /\u0001/];
let n = 0;
for (const f of files) {
  for (const { line, name, h } of allHovers(f).hovers) {
    n++;
    const where = `${path.basename(f)}:${line} ${name}\n${h}`;
    for (const re of INTERNAL) assert(!re.test(h), `internal name ${re} in ${where}`);
    const code = (h.match(/```python\n([\s\S]*?)```/g) || []).join("\n");
    const legend = h.split("\n").filter((l) => l.startsWith("*")).join(" ");
    if (code.includes("**?")) assert(legend.includes("`**?`"), `unexplained **? in ${where}`);
    if (/["'\w]\?:/.test(code)) assert(legend.includes('`"k"?:`'), `unexplained ?: in ${where}`);
    if (/(^|[\s:=(\[,|])\?($|[\s,)\]}|])/m.test(code.replace(/\*\*\?/g, ""))) assert(legend.includes("`?`"), `unexplained ? in ${where}`);
    for (const m of ["✗", "⚠", "◌"]) if (h.includes(`- ${m} `)) assert(legend.includes(m), `unexplained ${m} in ${where}`);
    // (a value block is the first thing after the title, or there is none)
    assert(h.startsWith(`**rowan** · \`${name}\`\n\n`), where);
  }
}
assert(n > 100, `only ${n} hovers checked`);
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`ok (${n} hovers checked)`);
console.log("\n--- sample hover for qkv (nanoGPT attention) ---\n" + qkv);
console.log("\n--- sample hover for r at line 9 ---\n" + r9);
console.log("\n--- sample hover for u at line 8 ---\n" + u8);
