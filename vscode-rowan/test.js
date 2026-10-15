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
// ---- several objects made on one line (one parser per call of build_parser): where from
const a002 = path.join(root, "tests", "apps", "small", "a002_deploy_cli.py");
const p28 = hover(indexResult(facts(a002)), fs.readFileSync(a002, "utf8").split("\n"), 28, "p");
assert(p28.includes("- one of 7 objects, all created on line 28 (by different calls or loop iterations)"), p28);
// (two Settings that differ only past the cut — retries=2 / retries=5 — are one line, not two alike)
const s57 = hover(indexResult(facts(a002)), fs.readFileSync(a002, "utf8").split("\n"), 57, "s");
const s57lines = s57.split("\n").filter((l) => /^(  |\| )Settings\(/.test(l)).map((l) => l.slice(2));
assert(s57lines.length === 3 && new Set(s57lines).size === 3, s57);
// (the legend explains only marks the shown value uses: `**?` and `[*]` are in the cut parts)
const a002idx = indexResult(facts(a002));
const a002src = fs.readFileSync(a002, "utf8").split("\n");
const set96 = hover(a002idx, a002src, 96, "settings");
assert(!set96.includes("`**?` other keys") && !set96.includes("`[*]` any element") && set96.includes("`a | b` one of these"), set96);
// (one access, two explanations of why it works: one line)
const cfg63 = hover(a002idx, a002src, 63, "cfg");
assert(cfg63.split("\n").filter((l) => l.startsWith('- ✓ `cfg["notify"]`')).length === 1, cfg63);
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
  // (a list too; a default the call only reads is not said to change)
  const z525 = path.join(root, "tests", "cases", "z525_skipped_call_parameter_escape.py");
  const r525 = JSON.parse(execFileSync(bin, [z525, "--facts"], { env: { ...process.env, ROWAN_LIMITS: "budget=1" }, maxBuffer: 64 * 1024 * 1024 }).toString());
  const s525 = fs.readFileSync(z525, "utf8").split("\n");
  const log = hover(indexResult(r525), s525, 46, "LOG");
  assert(log.includes("- may have been changed by `update(2)` (line 39, a call rowan no longer follows)"), log);
  const opts = hover(indexResult(r525), s525, 40, "OPTS");
  assert(!opts.includes("may have been changed") && opts.includes("✓ always works"), opts);
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

// ---- the owner's rnn (pytorch-playground): V = len(vocab) of unknown text names the dims
{
  const rnn = path.join(root, "tests", "torch", "playground", "p04_rnn.py");
  const src = fs.readFileSync(rnn, "utf8").split("\n");
  const idx = indexResult(facts(rnn));
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const hv = hover(idx, src, at("V: int = len(vocab)"), "V");
  assert(hv.includes("- not known statically; tensor dims equal to it are written `V`"), hv);
  const hl = hover(idx, src, at("    logits, _ = model(x)"), "logits");
  assert(hl.includes("Tensor[64, ?, V]"), hl);
}

// ---- the owner's GPT: its 96 heads within their budget, the model's output keeps its rank (and
// the batch window's length: train's unknown length is one value);
// a dim bound to `_` (`_ , T = ids.shape`) is not named `_` (it reads as "any dim")
{
  const gpt = path.join(root, "tests", "torch", "playground", "p06_gpt", "p06_gpt.py");
  const src = fs.readFileSync(gpt, "utf8").split("\n");
  const idx = indexResult(facts(gpt));
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const hl = hover(idx, src, at("        logits: Tensor = model(x)"), "logits");
  assert(hl.includes("Tensor[68, 64, V]"), hl);
  const hx = hover(idx, src, at("            x = block(x)"), "x");
  assert(hx.includes("Tensor[A, T, d]") && hx.includes("- `A, T` = `ids.shape`"), hx);
}

// ---- a list appended to on some paths: each list it may be, not `list[?]`; an item of a
// slice to a bound not known is the list's own
{
  const f = path.join(root, "tests", "cases", "z517_prefix_slice_unknown_bound.py");
  const src = fs.readFileSync(f, "utf8").split("\n");
  const idx = indexResult(facts(f));
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const hz = hover(idx, src, at("z1 = zs[1]"), "zs");
  assert(hz.includes('["x"] | ["x", "y"]'), hz);
  const hr = hover(idx, src, at("r1 = first[1]"), "r1");
  assert(hr.includes('"tags"'), hr);
}

// ---- a function one of several a dict holds, picked by a key rowan cannot compute: what
// fails in it is said to fail where the program picks it (a warning), not on every path
{
  const z512 = path.join(root, "tests", "cases", "z512_functions_picked_by_unknown_key.py");
  const line = fs.readFileSync(z512, "utf8").split("\n").findIndex((l) => l.includes('rec["b"]')) + 1;
  const vs = indexResult(facts(z512)).verdicts.get(line) || [];
  const v = vs.find((x) => x.tier === "possibly");
  assert(v && severityFor(v.tier) === "warning", JSON.stringify(vs));
  assert(diagnosticMessage(v).startsWith('key "b" is never set, where `CONVERTERS[kind_of(rec)](rec)` calls from_b'), diagnosticMessage(v));
}

// ---- keys found by an earlier read: said so, and one line for the keys of one access
{
  const found = path.join(tmp, "found.py");
  fs.writeFileSync(found, ["import sys", "text = sys.argv[1]", "idx = {c: i for i, c in enumerate(sorted(set(text)))}", "def enc(s):", "    return [idx[c] for c in s]", 'a = enc("ab")', 'b = enc("ab")', ""].join("\n"));
  const h = hover(indexResult(facts(found)), fs.readFileSync(found, "utf8").split("\n"), 5, "idx");
  assert(h.includes('- ✓ `idx[c]`: keys "a", "b" are there: an earlier read or test on this path found it'), h);
}

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
