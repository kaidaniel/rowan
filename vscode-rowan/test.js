// node test.js — checks the hover and diagnostic rendering against real --facts documents:
// what particular hovers say, and that every hover on several real programs has the owner's
// format (`name = value`, then Requirements / In Calls / Allocation Sites / Notes), no internal
// names, no marks for accesses that work, Requirements only of parameters (and of an input read
// on the line), every mark it uses explained in its Notes and every Note used above it.
"use strict";
const assert = require("assert");
const fs = require("fs");
const os = require("os");
const { execFileSync } = require("child_process");
const path = require("path");
const { renderHover, factsAt, indexResult, prettyValue, verdictsForWord, severityFor, diagnosticMessage, shorten, hoverServer, mergeResult, stopServers, runEngineServer, serverEvents, followArgs, sitePackagesOf } = require("./extension.js");

const root = path.join(__dirname, "..");
const bin = process.env.ROWAN_BIN || path.join(root, "rowan", "target", "release", "rowan");
const facts = (file, ...args) => JSON.parse(execFileSync(bin, [file, "--facts", ...args], { maxBuffer: 64 * 1024 * 1024 }).toString());

/** Every hover of a file, as the editor shows it (each place of a name on a line it has facts at). */
function allHovers(file) {
  const res = facts(file);
  const idx = indexResult(res);
  const out = [];
  for (const [key, all] of idx.facts) {
    const [line, name] = [Number(key.split(":")[0]), key.split(":").slice(1).join(":")];
    for (const start of new Set(all.flatMap((f) => f.cols || []))) {
      const fs_ = factsAt(all, start);
      const h = renderHover(name, fs_, verdictsForWord(idx.verdicts.get(line) || [], name, start, start + name.length));
      if (h) out.push({ line, name, h, facts: fs_ });
    }
  }
  return { res, hovers: out };
}

/** The hover of `name` on `line` of an indexed document, at its `nth` place on the line (0: the first). */
function hover(idx, src, line, name, nth = 0) {
  const re = new RegExp(`(^|[^A-Za-z0-9_])(${name})(?=$|[^A-Za-z0-9_])`, "g");
  let m;
  for (let i = 0; (m = re.exec(src[line - 1] || "")) && i < nth; i++);
  const start = m ? m.index + m[1].length : 0;
  return renderHover(name, factsAt(idx.facts.get(`${line}:${name}`) || [], start), verdictsForWord(idx.verdicts.get(line) || [], name, start, start + name.length));
}

// ---- a dict with a key set on some paths, a typo, an unknown argument
const a01 = path.join(root, "tests", "cases", "a01_conditional_key.py");
const a01src = fs.readFileSync(a01, "utf8").split("\n");
const a01idx = indexResult(facts(a01));
const r8 = hover(a01idx, a01src, 8, "r");
assert(r8.includes('```python\nr = {"id": u.id, "role"?: "admin"}\n```'), r8);
assert(r8.includes('**Notes**\n- `"k"?`: possible key'), r8);
assert(!r8.includes("**In Calls**") && !r8.includes("**Requirements**"), r8);
// ---- a name rebound from its own value: the hover shows what the line binds, not old | new
{
  const z614 = path.join(root, "tests", "cases", "z614_rebinding_shows_the_new_value.py");
  const src = fs.readFileSync(z614, "utf8").split("\n");
  const idx = indexResult(facts(z614));
  assert.strictEqual(hover(idx, src, 8, "x"), "```python\nx = 2\n```");
  assert.strictEqual(hover(idx, src, 10, "name"), '```python\nname = "Ada"\n```');
  assert.strictEqual(hover(idx, src, 12, "items"), "```python\nitems = [1, 2, 3]\n```");
}
// ---- a name at several places on a line: each place's own value (the second `cfg` has "host",
// the first does not; was `{**?} | {"host": ?, **?}` at both), the read in `x = x + 1` is 1
{
  const z628 = path.join(root, "tests", "cases", "z628_each_place_of_a_name_has_its_value.py");
  const src = fs.readFileSync(z628, "utf8").split("\n");
  const idx = indexResult(facts(z628));
  assert(hover(idx, src, 12, "cfg").startsWith("```python\ncfg = {**?}\n```"), hover(idx, src, 12, "cfg"));
  assert(hover(idx, src, 12, "cfg", 1).startsWith('```python\ncfg = {"host": ?, **?}\n```'), hover(idx, src, 12, "cfg", 1));
  assert.strictEqual(hover(idx, src, 15, "x"), "```python\nx = 2\n```");
  assert.strictEqual(hover(idx, src, 15, "x", 1), "```python\nx = 1\n```");
}
// ---- an unknown named by an expression, joined with a literal, is no longer that expression
// (`limit = max(1, y)` was `limit = retries - 1`)
{
  const z630 = path.join(root, "tests", "cases", "z630_joined_expression_is_not_the_expression.py");
  const src = fs.readFileSync(z630, "utf8").split("\n");
  const idx = indexResult(facts(z630));
  assert(hover(idx, src, 11, "y").startsWith("```python\ny = retries - 1\n```"), hover(idx, src, 11, "y"));
  assert(hover(idx, src, 12, "limit").startsWith("```python\nlimit = int(?)\n```"), hover(idx, src, 12, "limit"));
  assert(hover(idx, src, 14, "t").startsWith("```python\nt = int(?)\n```"), hover(idx, src, 14, "t"));
}
// ---- a loop's line shows the value the loop's fixpoint gives, not each round's as alternatives
// (`counts` was `{} | dict[?, 1] | dict[?, int(?)]`)
{
  const a04 = path.join(root, "tests", "cases", "a04_loop_keys.py");
  const src = fs.readFileSync(a04, "utf8").split("\n");
  const idx = indexResult(facts(a04));
  assert(hover(idx, src, 10, "counts").startsWith("```python\ncounts = dict[?, int(?)]\n```"), hover(idx, src, 10, "counts"));
}
// ---- several objects made on one line (one parser per call of build_parser): where from
const a002 = path.join(root, "tests", "apps", "small", "a002_deploy_cli.py");
const p28 = hover(indexResult(facts(a002)), fs.readFileSync(a002, "utf8").split("\n"), 28, "p");
assert.strictEqual(p28, "```python\np = <ArgumentParser>\n```\n\n**Allocation Sites**\n- one of 7 objects, all created on line 28 (by different calls or loop iterations)");
// (an object made in another file of the program: where)
{
  const b001 = path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "b001_shop.py");
  const h = hover(indexResult(facts(b001)), fs.readFileSync(b001, "utf8").split("\n"), 51, "short3");
  assert(h.includes("**Allocation Sites**\n- created on line 14 in shop/repo.py"), h);
}
// (a call site the main run records is listed once: not again from its caller's run for any call)
{
  const c001 = path.join(root, "tests", "apps", "large", "c001_platform_pkg", "c001_platform.py");
  const h = hover(indexResult(facts(c001)), fs.readFileSync(c001, "utf8").split("\n"), 29, "payload");
  assert(h.split("\n").filter((l) => l.endsWith(" on line 83 in dp/api.py.")).length === 1, h);
}
// (two Settings the calls give that differ only past the cut — retries=2 / retries=5 — are one
// entry, not two alike)
const s57 = hover(indexResult(facts(a002)), fs.readFileSync(a002, "utf8").split("\n"), 57, "s");
const s57calls = s57.split("\n").filter((l) => l.startsWith("- `Settings(") && l.endsWith(" on line 93."));
assert(s57calls.length === 3 && new Set(s57calls).size === 3, s57);
// (the legend explains only marks the shown value uses: `**?` and `[*]` are in the cut parts)
const a002idx = indexResult(facts(a002));
const a002src = fs.readFileSync(a002, "utf8").split("\n");
const set96 = hover(a002idx, a002src, 96, "settings");
// (Notes explain the marks the hover shows, not those of parts the cuts leave out)
const [body96, notes96] = set96.split("**Notes**");
assert(notes96.includes("- `**?`: other keys") === body96.includes("**?") && notes96.includes("- `[*]`: any element") === body96.includes("[*]") && notes96.includes("- `a | b`: either a or b"), set96);
// (an access that works has no mark: the hover says what may fail; `cfg` is `settings_from`'s
// parameter, so what it reads of it is said; the objects its calls pass were made on line 92)
const cfg63 = hover(a002idx, a002src, 63, "cfg");
assert(!cfg63.includes("✓") && cfg63.includes('**Requirements**\n- `cfg["name"]`') && cfg63.includes("**Allocation Sites**\n- one of 3 objects, all created on line 92"), cfg63);
// (an input read on its line: what the program reads of it, the only non-parameter with
// Requirements — what the JSON must have)
const cfg92 = hover(a002idx, a002src, 92, "cfg");
assert(cfg92.includes('```python\ncfg = json.loads(text)\n```\n\n**Requirements**\n- `cfg["name"]`\n\n**In Calls**'), cfg92);
// (an access that may fail is a diagnostic, which the editor shows in the same hover: not repeated)
const r9 = hover(a01idx, a01src, 9, "r");
assert(!r9.includes("⚠") && (a01idx.verdicts.get(9) || []).some((v) => severityFor(v.tier) === "warning" && v.message === 'key "role" is only set on some paths'), r9);
const r13 = hover(a01idx, a01src, 13, "r");
assert(!r13.includes("✗") && (a01idx.verdicts.get(13) || []).some((v) => severityFor(v.tier) === "error"), r13);
const u8 = hover(a01idx, a01src, 8, "u");
assert(u8.includes("```python\nu = ?\n```\n\n**Notes**") && !u8.includes("Requirements"), u8);
assert(u8.includes("- `?`: unknown (`u` is not defined in this file)"), u8);
// a parameter: any argument (the function's value for any call), what the program's call passes
// (`u`, line 8), what the function reads of it
const user2 = hover(a01idx, a01src, 2, "user");
assert(user2.includes("```python\nuser = (parameter)\n```") && user2.includes("**In Calls**\n- `u` on line 8.") && user2.includes("**Requirements**\n- `user.id`"), user2);
assert.strictEqual(renderHover("zzz", [], []), null);
// a key read only under a test of another argument: "(only if `admin`)", with its legend entry
// (at the parameter; a line in the body that reads it has no Requirements)
const z535 = path.join(root, "tests", "cases", "z535_requirements_under_parameter_tests.py");
const z535src = fs.readFileSync(z535, "utf8").split("\n");
const z535idx = indexResult(facts(z535));
const row11 = hover(z535idx, z535src, z535src.indexOf("def contract(row, admin):") + 1, "row");
assert(row11.includes('**Requirements**\n- `row["id"]` (only if `not admin`)\n- `row["role"]` (only if `admin`)'), row11);
assert(row11.includes("- `(only if x)`: needed only on calls where `x` holds of the arguments as passed"), row11);
// (in the body too, where it holds the argument of its name; a name built from it has none)
const rowBody = hover(z535idx, z535src, z535src.indexOf('        return row["role"]') + 1, "row");
assert(rowBody.includes("```python\nrow = (parameter)\n```\n\n**Requirements**\n") && rowBody.includes('- `row["role"]` (only if `admin`)'), rowBody);
const userBody = hover(z535idx, z535src, 9, "user");
assert(userBody && !userBody.includes("Requirements"), userBody);
// ---- a parameter in its `def`: what the function reads of it for any call (its contract),
// apart from what each of the program's calls passes and whether the reads work for it
{
  const z537 = path.join(root, "tests", "cases", "z537_parameter_contracts.py");
  const src = fs.readFileSync(z537, "utf8").split("\n");
  const idx = indexResult(facts(z537));
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const row = hover(idx, src, at("def make_user("), "row");
  assert(row.includes('```python\nrow = (parameter)\n```\n\n**Requirements**\n- `row["id"]`\n- `row["name"]`\n- `row["since"]` (only if `admin`)'), row);
  assert(row.includes(`- \`{"id": 1, "name": " ann "}\` on line ${at("a = make_user(")}.`) && row.includes(`- \`{"id": 2, "name": "bob"}\` on line ${at("b = make_user(")}: ✗ \`row["since"]\``), row);
  assert(!row.includes("✓") && row.includes("- ✗: fails on every path"), row);
  const order = hover(idx, src, at("def total("), "order");
  assert(order.includes('**Requirements**\n- `order["items"][*]["price"]`\n- `order["items"][*]["qty"]`'), order);
  assert(order.includes(` on line ${at("t = total(")}.`) && order.includes(` on line ${at("    u = total(")}: ✗ \`item["price"]\``), order);
  // (tensors: the shapes the function needs of it, relations to other parameters included)
  const g = path.join(root, "tests", "torch", "models", "m02_nanogpt.py");
  const gsrc = fs.readFileSync(g, "utf8").split("\n");
  const line = gsrc.findIndex((l, i) => l.includes("def forward(self, x)") && gsrc[i + 1].includes("B, T, C = x.size()")) + 1;
  const x = hover(indexResult(facts(g)), gsrc, line, "x");
  assert(x.includes("```python\nx = (parameter)\n```\n\n**Requirements**\n- `x.dim() == 3`\n- `x.shape[2] == self.n_embd`\n\n**In Calls**\n- `Tensor[3, 10, 32]` on line "), x);
}
// an alternative too wide for its line keeps whole items (`…` for the rest), not a cut word
assert.strictEqual(shorten('{"id": 1, "owner": "bo", "priority": 2, "tags": [], "title": "printer jam"}', 60), '{"id": 1, "owner": "bo", "priority": 2, "tags": [], …}');
assert.strictEqual(shorten('Ticket(id=1, title="a very long title that goes on and on and on")', 30), "Ticket(id=1, …)");
assert.strictEqual(shorten('x' + "y".repeat(70), 10), "xyyyyyyyy…");

// ---- a parameter needs the same in the body as at its `def`: what the function reads of it (not
// `cfg["verbose"]`, read of the input by the module code); an `__init__` whose class has a subclass too
{
  const z641 = path.join(root, "tests", "cases", "z641_parameter_requirements_in_body.py");
  const src = fs.readFileSync(z641, "utf8").split("\n");
  const idx = indexResult(facts(z641));
  for (const line of [22, 23]) {
    const h = hover(idx, src, line, "cfg");
    assert(h.includes('**Requirements**\n- `cfg["min_amount"]`\n\n') && !h.includes("verbose"), h);
  }
  for (const line of [14, 15]) {
    const h = hover(idx, src, line, "cfg");
    assert(h.includes('**Requirements**\n- `cfg["name"]`\n\n'), h);
  }
  // (a parameter in its body is one, also beside a call's input: was `cfg = cfg`, and
  // `{"name": cfg["name"], **?} | ?` where two runs for any call each had their object)
  for (const line of [15, 23]) assert(hover(idx, src, line, "cfg").startsWith("```python\ncfg = (parameter)\n```"), hover(idx, src, line, "cfg"));
}
// ---- values of a known builtin type: a range's items, enumerate's index, input() and what follows
// (they were `?`)
{
  const z642 = path.join(root, "tests", "cases", "z642_builtin_results_of_known_type.py");
  const src = fs.readFileSync(z642, "utf8").split("\n");
  const idx = indexResult(facts(z642));
  assert(hover(idx, src, 15, "i").startsWith("```python\ni = int(?)\n```"), hover(idx, src, 15, "i"));
  assert(hover(idx, src, 20, "j").startsWith("```python\nj = int(?)\n```"), hover(idx, src, 20, "j"));
  assert(hover(idx, src, 27, "parts").startsWith("```python\nparts = list[str(?)]\n```"), hover(idx, src, 27, "parts"));
  assert(hover(idx, src, 27, "parts").includes("- `str(?)`: a str not known statically"), hover(idx, src, 27, "parts"));
  // a word count: keys stored under str keys are str (was `dict[?, int(?)]`, `k = ?`)
  const z643 = path.join(root, "tests", "cases", "z643_keys_of_a_builtin_type.py");
  const src3 = fs.readFileSync(z643, "utf8").split("\n");
  const idx3 = indexResult(facts(z643));
  assert(hover(idx3, src3, 14, "counts").startsWith("```python\ncounts = dict[str(?), int(?)]\n```"), hover(idx3, src3, 14, "counts"));
  assert(hover(idx3, src3, 15, "k").startsWith("```python\nk = str(?)\n```"), hover(idx3, src3, 15, "k"));
}
// ---- a function's value for any call, apart from what the program's calls give (with their lines)
{
  const z532 = path.join(root, "tests", "cases", "z532_general_and_call_values.py");
  const src = fs.readFileSync(z532, "utf8").split("\n");
  const idx = indexResult(facts(z532));
  const y = hover(idx, src, 6, "y");
  assert(y.includes("```python\ny = 3 | x\n```") && y.includes("**In Calls**\n- `3` on line 44.\n- `7` on line 45."), y);
  assert(y.includes("- `a | b`: either a or b"), y);
  const b = hover(idx, src, 6, "b");
  assert(b.includes("```python\nb = (parameter) bool(?)\n```") && b.includes("**In Calls**\n- `False` on line 44.\n- `True` on line 45."), b);
  const m = hover(idx, src, 13, "m");
  assert(m.includes("```python\nm = n * 2\n```") && m.includes("**In Calls**\n- `10` on line 46.\n- `14` on line 46."), m);
  // a function nothing calls: only the value for any call
  const z = hover(idx, src, 27, "z");
  assert(z.includes("```python\nz = k + 1\n```") && !z.includes("In Calls"), z);
  // a method: on any instance (its `__init__` given any arguments), apart from the program's
  const v = hover(idx, src, 38, "v");
  assert(v.includes("```python\nv = 0 | int(?)\n```") && v.includes("**In Calls**\n- `4` on line 49."), v);
  assert(v.includes("- `int(?)`: an int not known statically"), v);
  // a method nothing calls: on any instance, not only one made with `__init__`'s defaults
  const a = hover(idx, src, 58, "a");
  assert(a.includes("```python\na = self.size * self.size\n```") && !a.includes("In Calls"), a);
}

// ---- unknowns of a known type: `str(?)`, from an annotation, a builtin, an operator
{
  const z533 = path.join(root, "tests", "cases", "z533_typed_unknowns.py");
  const src = fs.readFileSync(z533, "utf8").split("\n");
  const idx = indexResult(facts(z533));
  const label = hover(idx, src, 10, "label");
  assert(label.includes("```python\nlabel = str(?)\n```") && label.includes("**In Calls**\n- `\"AB\"` on line 45."), label);
  const size = hover(idx, src, 13, "size");
  assert(size.includes("```python\nsize = int(?)\n```"), size);
  const shown = hover(idx, src, 43, "shown");
  assert(shown.includes("```python\nshown = str(?)\n```") && shown.includes("- `str(?)`: a str not known statically") && !shown.includes("- `?`:"), shown);
}

// ---- a parameter's headline: `(parameter)`, with what any call may pass when something is
// known of it (its annotated type, its default, both); typed unknowns inside containers
{
  const f = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "rowan-params-")), "params.py");
  fs.writeFileSync(f, [
    "def f(a, b: int, c=3, d: int = 4, e: str | None = None):",
    "    return a, b, c, d, e",
    "",
    "",
    "def g(words: list):",
    "    sizes = {w: len(w) for w in words}",
    "    nums = [int(w) for w in words]",
    "    out = {\"sizes\": sizes, \"nums\": nums}",
    "    return out",
    "",
    "",
    "def h(hits: list[dict[str, str]], opt: dict[str, list[int | None]] | None, t: tuple[int, ...]):",
    "    return hits, opt, t",
    "",
    "",
    "f(1, 2)",
    "",
  ].join("\n"));
  const src = fs.readFileSync(f, "utf8").split("\n");
  const idx = indexResult(facts(f));
  const head = (name, line = 1) => hover(idx, src, line, name).split("\n")[1];
  assert.strictEqual(head("a"), "a = (parameter)");
  assert.strictEqual(head("b"), "b = (parameter) int(?)");
  assert.strictEqual(head("c"), "c = (parameter) 3 | ?");
  assert.strictEqual(head("d"), "d = (parameter) 4 | int(?)");
  assert.strictEqual(head("e"), "e = (parameter) None | str(?)");
  assert(hover(idx, src, 1, "c").includes("- `?`: unknown (whatever a call passes)"), hover(idx, src, 1, "c"));
  assert(hover(idx, src, 2, "b").includes("b = (parameter) int(?)"), hover(idx, src, 2, "b"));
  // (a dict whose keys only the program's stores under unknown keys put there; a list of
  // unknown length: what each holds, of its type)
  assert.strictEqual(head("sizes", 6), "sizes = dict[?, int(?)]");
  assert.strictEqual(head("nums", 7), "nums = list[int(?)]");
  const out = hover(idx, src, 8, "out");
  assert(out.includes('out = {"nums": list[int(?)], "sizes": dict[?, int(?)]}'), out);
  assert(out.includes("- `int(?)`: an int not known statically"), out);
  // (a builtin container annotation: with its parts' types)
  assert.strictEqual(head("hits", 12), "hits = (parameter) list[dict[str(?), str(?)]]");
  assert.strictEqual(head("opt", 12), "opt = (parameter) None | dict[str(?), list[int(?) | None]]");
  assert.strictEqual(head("t", 12), "t = (parameter) tuple[int(?), ...]");
  assert(hover(idx, src, 12, "hits").includes("- `str(?)`: a str not known statically"), hover(idx, src, 12, "hits"));
  // (typing notation for a container of unknown length or keys: each with its legend line)
  assert(hover(idx, src, 7, "nums").includes("- `list[x]`: a list of any length whose items are `x`"), hover(idx, src, 7, "nums"));
  assert(hover(idx, src, 6, "sizes").includes("- `dict[k, v]`: a dict of any size whose keys are `k` and values `v`"), hover(idx, src, 6, "sizes"));
  assert(hover(idx, src, 12, "t").includes("- `tuple[x, ...]`: a tuple of any length whose items are `x`"), hover(idx, src, 12, "t"));
  assert(!hover(idx, src, 12, "t").includes("`list[x]`"), hover(idx, src, 12, "t"));
  fs.rmSync(path.dirname(f), { recursive: true, force: true });
}

// ---- calls and unknowns from another file name it: a method of the hovered file that another
// module calls (`r.render(…)` on its line 5), an unknown made there (`json.loads(…)` on its line 9)
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "rowan-files-"));
  const f = path.join(dir, "main.py");
  fs.writeFileSync(path.join(dir, "helper.py"), ["import json", "", "", "def run(r):", '    return r.render({"title": "t"})', "", "", "def load():", "    return json.loads(input())", ""].join("\n"));
  fs.writeFileSync(f, ["import helper", "", "", "class Report:", "    def render(self, payload):", '        return payload["title"]', "", "", "def show(x):", "    return x", "", "", "helper.run(Report())", "show(helper.load())", ""].join("\n"));
  const src = fs.readFileSync(f, "utf8").split("\n");
  const res = facts(f);
  const idx = indexResult(res);
  // (the read of payload["title"] through `helper.run(Report())` on line 13, then `r.render(…)`
  // on helper.py's line 5)
  const title = res.verdicts.find((v) => v.line === 6 && v.key_str === "title");
  assert(title && title.via === "L13→helper.py:L5", JSON.stringify(title));
  const payload = hover(idx, src, 5, "payload");
  assert(payload.includes('- `{"title": "t"}` on line 5 in helper.py.'), payload);
  assert(hover(idx, src, 5, "self").includes("- `Report()` on line 5 in helper.py."), hover(idx, src, 5, "self"));
  const x = hover(idx, src, 9, "x");
  assert(x.includes("- `?` on line 14.") && x.includes("- `?`: unknown (the result of `json.loads(input())` on line 9 in helper.py)"), x);
  fs.rmSync(dir, { recursive: true, force: true });
}

// ---- a call rowan no longer follows (past its function's budget): said where it changes a value
{
  const z440 = path.join(root, "tests", "cases", "z440_past_budget_changes_globals.py");
  const res = JSON.parse(execFileSync(bin, [z440, "--facts"], { env: { ...process.env, ROWAN_LIMITS: "budget=2" }, maxBuffer: 64 * 1024 * 1024 }).toString());
  const h = hover(indexResult(res), fs.readFileSync(z440, "utf8").split("\n"), 58, "STATE");
  assert(h.includes("- `?`: unknown (may have been changed by `drop()` on line 57, a call rowan no longer follows)"), h);
  // (a list too; a default the call only reads is not said to change)
  const z525 = path.join(root, "tests", "cases", "z525_skipped_call_parameter_escape.py");
  const r525 = JSON.parse(execFileSync(bin, [z525, "--facts"], { env: { ...process.env, ROWAN_LIMITS: "budget=1" }, maxBuffer: 64 * 1024 * 1024 }).toString());
  const s525 = fs.readFileSync(z525, "utf8").split("\n");
  const log = hover(indexResult(r525), s525, 46, "LOG");
  assert(log.includes("may have been changed by `update(2)` on line 39, a call rowan no longer follows"), log);
  const opts = hover(indexResult(r525), s525, 40, "OPTS");
  assert(!opts.includes("may have been changed") && !opts.includes("✓"), opts);
}

// ---- diagnostics: only accesses that may fail, in words, with their call path
assert.strictEqual(severityFor("missing"), "error");
assert.strictEqual(severityFor("possibly"), "warning");
assert.strictEqual(severityFor("input"), null);
assert.strictEqual(severityFor("likely"), null);
const v13 = (a01idx.verdicts.get(13) || [])[0];
assert.strictEqual(diagnosticMessage(v13), 'key "rolle" is never set');
assert.strictEqual(diagnosticMessage({ message: "m", chain: [5, 9] }), "m (via L5→L9)");
assert.strictEqual(diagnosticMessage({ message: "m", chain: [5, 9], via: "L5→helper.py:L9" }), "m (via L5→helper.py:L9)");

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
// a one-item tuple keeps its comma, also when cut or broken over lines
assert.deepStrictEqual(prettyValue("E(args=(str(?),)) | ?"), ["E(args=(str(?),)) | ?"]);
assert.deepStrictEqual(prettyValue("[(1,), (2, 3)]"), ["[(1,), (2, 3)]"]);
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
assert(x1.includes("```python\nx1 = b\n```"), x1);
assert(!x1.includes("**?") && !x1.includes("unknown"), x1);
const u8z = hover(zi, zsrc, 8, "_");
assert(u8z.includes("```python\n_ = a(b)(a)\n```"), u8z);

// ---- tensors (nanoGPT's attention): the shape in the function's own terms, dims named by
// the locals that hold them, the line's requirement, what calls gave
const g = path.join(root, "tests", "torch", "models", "m02_nanogpt.py");
const gsrc = fs.readFileSync(g, "utf8").split("\n");
const gi = indexResult(facts(g));
const qkv = hover(gi, gsrc, 64, "qkv");
assert(qkv.includes("```python\nqkv = Tensor[B, T, 3*self.n_embd]\n```"), qkv);
// (Notes: only the dims the value uses — no `C`; what the line needs of `x` is said at `x`)
assert(qkv.endsWith("**Notes**\n- `B`: `x.shape[0]`\n- `T`: `x.shape[1]`") && !qkv.includes("Requirements"), qkv);
assert(qkv.includes("**In Calls**\n- `Tensor[3, 10, 96]` on line "), qkv);
const gx = hover(gi, gsrc, 64, "x");
assert(gx.includes("x = Tensor[B, T, C]") && gx.includes("- `B, T, C`: `x.shape`"), gx);

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
  "e = {'a': 1, 'b': 2}",
  "del e['b']",
  "n = len(e)",
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
const hd = renderHover("d", factsAt(ki.facts.get("12:d"), dq), verdictsForWord(ki.verdicts.get(12), "d", dq, dq + 1));
assert(hd.includes('```python\nd = {"k": 1}\n```') && !hd.includes("✗") && diagnosticMessage(vq) === 'key "q" is never set', hd);
// (the other `d` on the line, being assigned, has no access)
assert(!hover(ki, ksrc, 12, "d").includes("✗"), hover(ki, ksrc, 12, "d"));
// (where an object was made, when not on the hovered line nor by the name's own assignment;
// a key it never has here, unless the headline shows the dict whole: `e = {"a": 1}` on line 15
// says both)
assert(!hover(ki, ksrc, 15, "e").includes("Allocation Sites"), hover(ki, ksrc, 15, "e"));
assert(!hover(ki, ksrc, 13, "e").includes("Allocation Sites"), hover(ki, ksrc, 13, "e"));
assert(hover(a01idx, a01src, 9, "r").includes("**Allocation Sites**\n- created on line 2\n"), hover(a01idx, a01src, 9, "r"));
assert(!hover(a01idx, a01src, 5, "d").includes("Allocation Sites"), hover(a01idx, a01src, 5, "d"));
{
  const pm = path.join(root, "tests", "pandas", "models", "pm01_sales_etl.py");
  const h = hover(indexResult(facts(pm)), fs.readFileSync(pm, "utf8").split("\n"), 71, "df");
  assert(h.includes('**Allocation Sites**\n- one of 3 objects, all created on line 67 (by different calls or loop iterations)\n- never has key "discount" here'), h);
}

// ---- the owner's rnn (pytorch-playground): V = len(vocab) of unknown text names the dims
{
  const rnn = path.join(root, "tests", "torch", "playground", "p04_rnn.py");
  const src = fs.readFileSync(rnn, "utf8").split("\n");
  const idx = indexResult(facts(rnn));
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const hv = hover(idx, src, at("V: int = len(vocab)"), "V");
  assert(hv.includes("```python\nV = ?\n```") && hv.includes("- `?`: unknown (tensor dims equal to it are written `V`)"), hv);
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
  assert(hx.includes("x = Tensor[A, T, d]") && hx.includes("- `A, T`: `ids.shape`"), hx);
}

// ---- a FastAPI handler's parameter: the requests that reach it, not one its validation rejects
{
  const f = path.join(root, "tests", "fastapi", "small", "f042_rejected_request_never_reaches_handler.py");
  const src = fs.readFileSync(f, "utf8").split("\n");
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const h = hover(indexResult(facts(f)), src, at("def create("), "order");
  assert(h.includes(`\`OrderIn(item="tea", qty=4, size="M")\` on line ${at("ok = ")}.`) && h.includes(`on line ${at("dup = ")}.`) && !h.includes(`on line ${at("bad = ")}`) && !h.includes("qty=0"), h);
}
// ---- `m = m.copy()` of a parameter rowan knows nothing of: as unknown, not a dict (`{**?}`)
{
  const pm = path.join(root, "tests", "pandas", "models", "pm01_sales_etl.py");
  const src = fs.readFileSync(pm, "utf8").split("\n");
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const h = hover(indexResult(facts(pm)), src, at("    m = m.copy()"), "m");
  assert(h.startsWith("```python\nm = ?\n```"), h);
}
// ---- a module's defaults only copied (`{**DEFAULTS, …}`): no code changes them, so for any
// call their keys are there and their values known
{
  const f = path.join(root, "tests", "cases", "z616_module_defaults_copied_not_changed.py");
  const src = fs.readFileSync(f, "utf8").split("\n");
  const at = (pre) => src.findIndex((l) => l.startsWith(pre)) + 1;
  const h = hover(indexResult(facts(f)), src, at("    cfg = {**DEFAULTS"), "cfg");
  assert(h.includes('"retries": 3 | json.loads(text)["retries"]') && !h.includes('"retries"~'), h);
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
  assert(h.includes("```python\nidx = dict[?, int(?)]") && !h.includes("✓"), h); // (keys only possible, of values not known, are `**?`'s)
}

// ---- a parameter annotated with a dataclass: its fields (its bases' too) as annotated
const z618 = path.join(root, "tests", "cases", "z618_annotated_field_classes.py");
const z618src = fs.readFileSync(z618, "utf8").split("\n");
const hIt = hover(indexResult(facts(z618)), z618src, z618src.findIndex((l) => l.startsWith("def describe(")) + 1, "it");
for (const part of ["ident=int(?)", "label=str(?)", "note=None | str(?)", "tags=list[str(?)]"]) assert(hIt.includes(part), hIt);

// ---- a parameter's own argument beside its default, in its body and in In Calls (a framework's
// request: the annotated type, never the parameter's name as a value)
const f043 = path.join(root, "tests", "fastapi", "small", "f043_request_parameters_as_annotated.py");
const f043src = fs.readFileSync(f043, "utf8").split("\n");
const f043idx = indexResult(facts(f043));
const defLine = f043src.findIndex((l) => l.startsWith("def read_run(")) + 1;
const hLimit = hover(f043idx, f043src, defLine, "limit");
assert(hLimit.startsWith("```python\nlimit = (parameter) 10 | int(?)\n```") && hLimit.includes("- `10 | int(?)` on line ") && !/`[^`]*\blimit\b[^`]*` on line/.test(hLimit), hLimit);
const hLimit2 = hover(f043idx, f043src, defLine + 3, "limit");
assert(hLimit2.startsWith("```python\nlimit = (parameter) 10 | int(?)\n```"), hLimit2);

// ---- exceptions as Python writes them: their arguments by position, `__cause__` when set
const z619 = path.join(root, "tests", "cases", "z619_exception_values.py");
const z619src = fs.readFileSync(z619, "utf8").split("\n");
const z619idx = indexResult(facts(z619));
const errLine = z619src.findIndex((l) => l.trim() === "err = e") + 1;
assert(hover(z619idx, z619src, errLine, "err").startsWith('```python\nerr = AppError("too big")\n```'), hover(z619idx, z619src, errLine, "err"));
const codedLine = z619src.findIndex((l) => l.trim() === "coded = c") + 1;
assert(hover(z619idx, z619src, codedLine, "coded").startsWith('```python\ncoded = Coded("negative", __cause__=KeyError("k"), code=400)\n```'), hover(z619idx, z619src, codedLine, "coded"));

// ---- every hover of several real programs: no internal names, every mark explained
// ---- examples/etl_report.py: dataclass instances the program builds from unknown data hold
// what the field annotations say (`float` is not trusted: an int passes for one); a list of
// them in typing notation, with its legend line (not a list of exactly one Summary)
const etl = path.join(root, "examples", "etl_report.py");
{
  const src = fs.readFileSync(etl, "utf8").split("\n");
  const idx = indexResult(facts(etl));
  const s109 = hover(idx, src, 109, "s");
  assert(s109.startsWith("```python\ns = Summary(n_orders=int(?), region=str(?), total=?)\n```"), s109);
  const sum109 = hover(idx, src, 109, "summaries");
  assert(sum109.startsWith("```python\nsummaries = list[Summary(n_orders=int(?), region=str(?), total=?)]\n```"), sum109);
  assert(sum109.includes("- `list[x]`: a list of any length whose items are `x`"), sum109);
  assert(!s109.includes("`list[x]`"), s109);
}
const files = [
  a01, z, g, kinds, z618, f043, z619, etl,
  path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "shop", "repo.py"),
  path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "b001_shop.py"),
  path.join(root, "tests", "apps", "large", "c001_platform_pkg", "dp", "pipeline.py"),
  path.join(root, "tests", "pandas", "models", fs.readdirSync(path.join(root, "tests", "pandas", "models")).filter((f) => f.endsWith(".py"))[0]),
];
const INTERNAL = [/PH@/, /\bM\d+:L\d+/, /#\d{3,}/, /\bparam [\w.]+\.\w/, /\b(item|attr)\[/, /\bdef \w/, /\bresult of [^`]*@L/, /\u0001/];
/** What each notation a Note explains looks like where it is used. */
const USES = {
  "?": (t) => /(^|[\s:=(\[,|`])\?($|[\s,)\]}|`])/m.test(t.replace(/\*\*\?/g, "").replace(/\b\w+\(\?\)/g, "T")),
  '"k"?': (t) => /["'\w]\?[:=]/.test(t),
  '"k"~': (t) => /["'\w]~[:=]/.test(t),
  "**?": (t) => t.includes("**?"),
  "a | b": (t) => / \| |(^|\n)\s*\| /.test(t),
  "Tensor[*]": (t) => /\bTensor\[\*\]/.test(t),
  "[*]": (t) => /\[\*\]/.test(t.replace(/Tensor\[\*\]/g, "")),
  "…": (t) => t.includes("…"),
  "(only if x)": (t) => t.includes("(only if `"),
  "list[x]": (t) => /(^|[^\w.])list\[/.test(t),
  "set[x]": (t) => /(^|[^\w.])set\[/.test(t),
  "tuple[x, ...]": (t) => /(^|[^\w.])tuple\[[^\n]*, \.\.\.\]/.test(t),
  "dict[k, v]": (t) => /(^|[^\w.])dict\[/.test(t),
};
const MARKS = ["✗", "⚠~", "⚠", "◌"];
/** How a whole program's or a function's run was cut: its Note's opening words. */
const CUT_NOTE = /^- (analysed on demand: |from the module's own code only |the whole program's analysis was cut )/;
/**
 * The owner's format: `name = value` in a code block, then only the sections Requirements, In
 * Calls, Allocation Sites and Notes, in that order, each a non-empty list; Requirements only of
 * a parameter (`(parameter)`) or of an input the line reads (`facts`); no mark for an access
 * that works; every mark and notation the hover shows explained in its Notes, and every Note
 * about something the hover shows (a symbol, a notation, a mark: above it, or in another
 * Note's description), the Note on a cut analysis last and only for one.
 */
function checkFormat(h, name, where, facts = []) {
  for (const re of INTERNAL) assert(!re.test(h), `internal name ${re} in ${where}`);
  const parts = h.split("\n\n");
  assert(parts[0].startsWith("```python\n" + name + " = ") && parts[0].endsWith("\n```"), `headline in ${where}`);
  const titles = parts.slice(1).map((p) => p.split("\n")[0]);
  const order = ["**Requirements**", "**In Calls**", "**Allocation Sites**", "**Notes**"];
  assert(titles.every((t, k) => order.includes(t) && (k === 0 || order.indexOf(titles[k - 1]) < order.indexOf(t))), `sections in ${where}`);
  for (const p of parts.slice(1)) assert(p.split("\n").slice(1).every((l) => l.startsWith("- ")) && p.includes("\n- "), `section in ${where}`);
  assert(!h.includes("✓"), `a mark for an access that works in ${where}`);
  if (h.includes("**Requirements**")) {
    // (a parameter: at its `def`, or holding the argument of its name; an input read on the line)
    const objs = (f) => [...(f.objects || []), ...(f.general_unknowns || [])];
    const param = facts.some((f) => f.param || objs(f).some((o) => o.kind === "unknown" && (o.from || []).some((t) => t.startsWith(`argument \`${name}\` of `))));
    const input = facts.some((f) => objs(f).some((o) => (o.origin && o.origin.line === f.line) || (o.from || []).some((t) => t.endsWith(`(line ${f.line})`))));
    assert(param || input, `Requirements of a name that is not a parameter in ${where}`);
  }
  const [body, notesPart = ""] = h.split("**Notes**");
  const notes = notesPart.split("\n").filter((l) => l.startsWith("- "));
  const code = body.replace(/\(only if `[^`]*`\)/g, "");
  // (every mark and notation used is explained)
  const explained = (key) => notes.some((l) => l.startsWith(`- \`${key}\`:`));
  for (const [key, used] of Object.entries(USES)) if (used(key === "(only if x)" ? body : code)) assert(explained(key), `unexplained ${key} in ${where}`);
  for (const m of (code.match(/\b(int|str|bool|bytes|float|list|dict|set|tuple|frozenset)\(\?\)/g) || [])) assert(explained(m), `unexplained ${m} in ${where}`);
  for (const m of MARKS) if (body.replace(/⚠~/g, m === "⚠" ? "" : "⚠~").includes(m)) assert(notes.some((l) => l.startsWith(`- ${m}: `)), `unexplained ${m} in ${where}`);
  // (every Note is about something shown: above it, or in another Note's description)
  notes.forEach((l, k) => {
    const rest = [body, ...notes.filter((_, j) => j !== k).map((x) => x.replace(/^- `[^`]*`: /, ""))].join("\n");
    if (CUT_NOTE.test(l)) return assert(k === notes.length - 1 && facts.some((f) => f.run), `cut note not last, or for a whole run, in ${where}`);
    const mark = MARKS.find((m) => l.startsWith(`- ${m}: `));
    if (mark) return assert(body.includes(mark), `unused ${mark} in ${where}`);
    const key = (/^- `([^`]*)`: /.exec(l) || [])[1];
    assert(key !== undefined, `a Note without a key in ${where}`);
    if (USES[key]) return assert(USES[key](rest), `unused ${key} in ${where}`);
    if (/^\w+\(\?\)$/.test(key)) return assert(rest.includes(key), `unused ${key} in ${where}`);
    // (a symbol: a name the value mentions, with what is known of it)
    const word = (w) => new RegExp(`(^|[^\\w.])${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^\\w])`).test(body);
    assert(key.split(", ").some(word), `unused symbol ${key} in ${where}`);
  });
}
let n = 0;
for (const f of files) {
  for (const { line, name, h, facts: fs_ } of allHovers(f).hovers) {
    n++;
    checkFormat(h, name, `${path.basename(f)}:${line} ${name}\n${h}`, fs_);
  }
}
assert(n > 100, `only ${n} hovers checked`);
// ---- on demand (`rowan serve`'s `hover`: one function run alone, for any call): what the
// hovers of real programs say, with the same rules
async function onDemand() {
  let m = 0;
  // (a01's build: any `user`, the code reads `user.id`; "role" set only when `admin` holds)
  const b = await hoverServer(bin, a01, 3, [], 60);
  assert.deepStrictEqual(b.span, [1, 5]);
  const bidx = indexResult(b.result);
  const d5 = hover(bidx, a01src, 5, "d");
  assert(d5.includes('```python\nd = {"id": user.id, "role"?: "admin"}\n```') && d5.includes('- `"k"?`: possible key'), d5);
  const u2 = hover(bidx, a01src, 2, "user");
  assert(u2.includes("```python\nuser = (parameter)\n```\n\n**Requirements**\n- `user.id`") && !u2.includes("In Calls"), u2);
  // (its callees past the work a run on demand may do: the note says their calls were unknown)
  const cutCallees = indexResult({ ...b.result, analysis: { ...b.result.analysis, callees_cut: 16384 } });
  const d5c = hover(cutCallees, a01src, 5, "d");
  assert(/\n- analysed on demand: this function alone, for any call, in [^\n]*, because the whole program's analysis took too long; past 16,384 statements, the calls made by the functions it calls were taken as unknown code$/.test(d5c), d5c);
  // (and the engine says when it happened: `main` calls functions that call others, past a
  // budget of 5 statements)
  const deploy = path.join(root, "tests", "apps", "small", "a002_deploy_cli.py");
  const runs = execFileSync(bin, ["hovers", deploy, "--only", "main"], { env: { ...process.env, ROWAN_LIMITS: "budget=5" } }).toString().trim().split("\n").slice(1).map((l) => JSON.parse(l));
  assert(runs.length === 1 && runs[0].callees_cut === true, JSON.stringify(runs));
  // (a second hover in the same function: the cached document, merged once into the index)
  const again = await hoverServer(bin, a01, 4, [], 60);
  assert.deepStrictEqual(again.span, [1, 5]);
  mergeResult(bidx, { facts: [], verdicts: [] });
  for (const f of [path.join(root, "tests", "apps", "medium", "b001_shop_pkg", "shop", "repo.py"), path.join(root, "tests", "apps", "small", "a002_deploy_cli.py"), path.join(root, "tests", "apps", "large", "c001_platform_pkg", "dp", "pipeline.py")]) {
    const src = fs.readFileSync(f, "utf8").split("\n");
    const idx = indexResult({ facts: [], verdicts: [] });
    const spans = [];
    for (let line = 1; line <= src.length; line++) {
      if (!/^\s*def /.test(src[line - 1]) || spans.some(([a, z2]) => a <= line && line <= z2)) continue;
      let r;
      try {
        r = await hoverServer(bin, f, line + 1, [], 60);
      } catch (e) {
        continue; // (a nested def of a function not made by the module's own code)
      }
      spans.push(r.span);
      mergeResult(idx, r.result);
    }
    assert(spans.length > 3, `${f}: ${spans.length} functions`);
    for (const [key, fs_] of idx.facts) {
      const [line, name] = [Number(key.split(":")[0]), key.split(":").slice(1).join(":")];
      const h = hover(idx, src, line, name);
      if (!h) continue;
      m++;
      const where = `${path.basename(f)}:${line} ${name} (on demand)\n${h}`;
      checkFormat(h, name, where, fs_);
      assert(!h.includes("**In Calls**"), where);
      // (said last: the function was analysed alone, for any call, and how long it took)
      assert(/\n- analysed on demand: this function alone, for any call, in [^\n]* \([\d,]+ statements?\), because the whole program's analysis took too long(; past [\d,]+ statements, the calls made by the functions it calls were taken as unknown code)?$/.test(h), where);
    }
  }
  stopServers();
  assert(m > 50, `only ${m} on-demand hovers checked`);
  return { m, d5 };
}

// ---- an early answer (`analyse` of a program whose whole analysis takes seconds): the module
// code's document at once (on demand, `pending`), the whole program's as an event later
async function early() {
  // (module code that is quick; functions nothing calls whose nested loops of known items,
  // unrolled, take the whole analysis seconds)
  const loops = (op) => `    out = {}\n    for i in range(16):\n        for j in range(16):\n            for k in range(16):\n                out[(i, j, k)] = a * i ${op} b * j + k\n    return out\n`;
  const f = path.join(tmp, "slow_whole_program.py");
  const grids = Array.from({ length: 16 }, (_, n) => `def grid${n}(a, b):\n${loops(n % 2 ? "+" : "-")}\n\n`).join("");
  fs.writeFileSync(f, `ROWS = [{"id": i, "name": "n%d" % i} for i in range(16)]\n\n\ndef label(row):\n    return row["name"] + "!"\n\n\n${grids}first = label(ROWS[0])\n`);
  const got = new Promise((resolve) => {
    const h = (m) => {
      if (m.event === "analysed" && m.path === f) {
        serverEvents.delete(h);
        resolve(m);
      }
    };
    serverEvents.add(h);
  });
  const t0 = Date.now();
  const res = await runEngineServer(bin, f, [], 60, []);
  const first = Date.now() - t0;
  // (a machine fast enough to analyse it whole within the wait: one answer)
  // (the whole program's analysis cut at its deadline: its hovers say so, last; the grids'
  // entry runs take seconds, so a run past 1 ms is cut even when the module code is done by then)
  const fsrc = fs.readFileSync(f, "utf8").split("\n");
  const cutRes = JSON.parse(execFileSync(bin, [f, "--facts"], { env: { ...process.env, ROWAN_LIMITS: "ms=1" }, maxBuffer: 64 * 1024 * 1024 }).toString());
  assert(cutRes.analysis.cut, JSON.stringify(cutRes.analysis));
  const cutRows = hover(indexResult(cutRes), fsrc, 1, "ROWS");
  assert(/\n- the whole program's analysis was cut at its 1 ms deadline \(after [\d,]+ statements?\): code it had not reached by then is taken as unknown code, so values may say less than they could$/.test(cutRows), cutRows);
  checkFormat(cutRows, "ROWS", cutRows, indexResult(cutRes).facts.get("1:ROWS"));
  if (!res.pending) return `whole program at once (${first} ms)`;
  assert(res.on_demand && first < 5000, `early answer: on demand at once (${first} ms)`);
  // (the module code's document: said so, while the whole program's analysis goes on)
  const rows = hover(indexResult(res), fsrc, 1, "ROWS");
  assert(rows.endsWith("\n- from the module's own code only (its functions are analysed when you hover them), because the whole program's analysis is still running"), rows);
  const ev = await Promise.race([got, new Promise((r) => setTimeout(() => r(null), 60000))]);
  assert(ev && ev.ok && !ev.result.on_demand && ev.result.facts.length > 0, "the whole program's document follows");
  return `module code at ${first} ms, whole program at ${ev.ms} ms`;
}

// ---- installed packages followed from an interpreter's site-packages (`rowan.followPackages`):
// the interpreter is asked for its site-packages, `rowan serve` gets `--follow` and
// `--site-packages`, and a name an installed package defines is known (unknown otherwise)
async function follow() {
  assert.deepStrictEqual(followArgs([], ["/s"]), []);
  assert.deepStrictEqual(followArgs(["*"], ["/a", "/b"]), ["--follow", "*", "--site-packages", "/a", "--site-packages", "/b"]);
  assert.deepStrictEqual(followArgs(["a", " ", "b"], []), ["--follow", "a,b"]);
  const site = path.join(root, "tests", "cases", "n99_libbudget", "site");
  // (an interpreter stand-in printing what `sysconfig` would: a directory that does not exist is left out)
  const py = path.join(tmp, "python-stand-in");
  fs.writeFileSync(py, `#!/bin/sh\necho '${JSON.stringify([site, path.join(tmp, "no-such-dir")])}'\n`, { mode: 0o755 });
  const dirs = await sitePackagesOf(py);
  assert.deepStrictEqual(dirs, [site]);
  assert.deepStrictEqual(await sitePackagesOf(path.join(tmp, "no-such-python")), []);
  const f = path.join(site, "..", "n99_lib_budgets.py");
  const src = fs.readFileSync(f, "utf8").split("\n");
  const on = hover(indexResult(await runEngineServer(bin, f, followArgs(["partlib"], dirs), 60, [])), src, 14, "name");
  assert.strictEqual(on, '```python\nname = "partlib"\n```');
  const off = hover(indexResult(await runEngineServer(bin, f, [], 60, [])), src, 14, "name");
  // (not followed: a library's attribute, which the hover leaves to the language server)
  assert(!(off || "").includes('"partlib"'), off);
}

onDemand().then(async ({ m, d5 }) => {
const e = await early();
await follow();
stopServers();
console.log(`early answer: ${e}`);
fs.rmSync(tmp, { recursive: true, force: true });
console.log(`ok (${n} hovers checked, ${m} on demand)`);
console.log("\n--- sample hover for qkv (nanoGPT attention) ---\n" + qkv);
console.log("\n--- sample hover for r at line 9 ---\n" + r9);
console.log("\n--- sample hover for u at line 8 ---\n" + u8);
console.log("\n--- sample hover for row in contract (z535) ---\n" + row11);
console.log("\n--- sample hover on demand for d at line 5 ---\n" + d5);
}, (e) => {
  stopServers();
  console.error(e);
  process.exit(1);
});
