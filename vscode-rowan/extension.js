// rowan VS Code extension: runs `rowan FILE --facts` and shows what rowan inferred about a
// variable on hover (`name = value` as Python; what the code needs of it, what the program's
// calls give, what the marks mean) and the accesses that may fail as diagnostics.  Plain JavaScript, no build step.
"use strict";

const path = require("path");
const fs = require("fs");
const { execFile, spawn } = require("child_process");

let vscode = null;
try {
  vscode = require("vscode");
} catch (e) {
  vscode = null; // running outside VS Code (tests)
}

// ----------------------------------------------------------------- rendering (pure)

/**
 * The mark of a call's check of a parameter (the engine's `--facts` verdict tiers), worst first.
 * Accesses that work (`proven`, `likely`) get no mark: the hover says only what may fail.
 */
const MARK = { missing: "✗", possibly: "⚠", "likely-missing": "⚠~", input: "◌" };
const CHECK_ORDER = ["missing", "possibly", "likely-missing", "input", "likely", "proven"];
/** What each mark means (Notes). */
const MARK_TEXT = {
  "⚠": "fails on some paths",
  "✗": "fails on every path",
  "⚠~": "fails unless code rowan cannot see set the key",
  "◌": "works if the input has what the function reads",
};

/**
 * The notation of values (as the engine prints them): a test on the text, the mark, and its
 * meaning (Notes). `?` is said apart: with where its unknowns come from.
 */
const TYPED = /\b(int|str|bool|bytes|float|list|dict|set|tuple|frozenset)\(\?\)/g;
const UNKNOWN = (t) => /(^|[\s:=(\[,|`])\?($|[\s,)\]}|`])/.test(t.replace(/\*\*\?/g, "").replace(TYPED, "T"));
/** The builtin types of the unknowns `t` shows (`int(?)`, `str(?)`), each with its Note. */
const typedNotes = (t) => [...new Set([...t.matchAll(TYPED)].map((m) => m[1]))].map((ty) => `\`${ty}(?)\`: ${/^[aeiou]/.test(ty) ? "an" : "a"} ${ty} not known statically`);
const NOTATION = [
  [(t) => /["'\w]\?[:=]/.test(t), '"k"?', "possible key"],
  [(t) => /["'\w]~[:=]/.test(t), '"k"~', "there unless code rowan cannot see removed it"],
  [(t) => t.includes("**?"), "**?", "other keys may exist"],
  [(t) => / \| |(^|\n)\s*\| /.test(t), "a | b", "either a or b (depending on the path or the call)"],
  [(t) => /\bTensor\[\*\]/.test(t), "Tensor[*]", "a tensor of unknown number of dimensions"],
  [(t) => /\[\*\]/.test(t.replace(/\bTensor\[\*\]/g, "")), "[*]", "any element"],
  [(t) => t.includes("…"), "…", "not shown"],
  [(t) => t.includes(" (only if `"), "(only if x)", "needed only on calls where `x` holds of the arguments as passed"],
];

/**
 * Values are printed as Python with a few additions (see NOTATION): `?` an unknown value;
 * `{"a": 1, "b"?: ?, "c"~: ?, **?}` a dict with "a" always, "b" on some paths, "c" unless
 * unknown code removed it, and possibly other keys; `Name(a=1, **?)` an object; `list[X]` a
 * list of unknown length; `…` what the printer left out; `<Match>` an object by its type.
 */
const VALUE_WIDTH = 60;
const CLOSE_OF = { "{": "}", "[": "]", "(": ")" };

/**
 * Parse a rendered value (`Repo(items={1: …}, name="x")`) into items: each item is a list
 * of parts, a part is text or a bracketed node `{open, close, items}`.
 */
function parseValue(s) {
  let i = 0;
  function items(close) {
    const out = [];
    let parts = [];
    let buf = "";
    const flush = () => {
      if (buf) parts.push(buf);
      buf = "";
    };
    const endItem = () => {
      flush();
      if (parts.length) out.push(parts);
      parts = [];
    };
    while (i < s.length) {
      const c = s[i];
      if (c === '"' || c === "'") {
        let j = i + 1;
        while (j < s.length && s[j] !== c) j += s[j] === "\\" ? 2 : 1;
        buf += s.slice(i, j + 1);
        i = j + 1;
      } else if (CLOSE_OF[c]) {
        flush();
        i++;
        parts.push({ open: c, close: CLOSE_OF[c], items: items(CLOSE_OF[c]) });
      } else if (c === close) {
        i++;
        endItem();
        return out;
      } else if (c === "," && close !== null) {
        i++;
        while (s[i] === " ") i++;
        endItem();
      } else {
        buf += c;
        i++;
      }
    }
    endItem();
    return out;
  }
  return items(null);
}

function flatParts(parts) {
  return parts.map((p) => (typeof p === "string" ? p : p.open + p.items.map(flatParts).join(", ") + p.close)).join("");
}

/** The top-level alternatives of a value (`A | B`), outside brackets and strings. */
function alternatives(s) {
  const out = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '"' || c === "'") {
      let j = i + 1;
      while (j < s.length && s[j] !== c) j += s[j] === "\\" ? 2 : 1;
      i = j;
    } else if (CLOSE_OF[c]) depth++;
    else if (c === "}" || c === "]" || c === ")") depth--;
    else if (depth === 0 && s.startsWith(" | ", i)) {
      out.push(s.slice(start, i));
      start = i + 3;
      i += 2;
    }
  }
  out.push(s.slice(start));
  return out;
}

/**
 * Lines of a value no wider than `width` where possible: a structure that fits stays on one
 * line, a larger one with several items has one item per line, indented two spaces per
 * level; a value that is one of several long alternatives has one per line (`| B`).
 */
function prettyValue(text, width = VALUE_WIDTH) {
  if (text.length > width) {
    const alts = alternatives(text);
    if (alts.length > 1) return alts.flatMap((a, k) => prettyValue(a, width - 2).map((l, j) => (j === 0 ? (k ? "| " : "  ") : "  ") + l));
  }
  const lines = [];
  function emit(parts, indent, cur) {
    // `cur`: the line being built (already indented); returns the line still open
    for (const p of parts) {
      if (typeof p === "string") {
        cur += p;
        continue;
      }
      const flat = flatParts([p]);
      // (a call or subscript with one argument stays whole: `json.loads(text)["id"]`; a
      // container with one long item breaks)
      const container = p.open !== "(" && !/[\w)\]"'.]$/.test(cur);
      if (cur.length + flat.length <= width || p.items.length === 0 || (p.items.length < 2 && !container)) {
        cur += flat;
        continue;
      }
      lines.push(cur + p.open);
      const pad = " ".repeat(indent + 2);
      p.items.forEach((item, k) => {
        const last = emit(item, indent + 2, pad);
        lines.push(last + (k < p.items.length - 1 ? "," : ""));
      });
      cur = " ".repeat(indent) + p.close;
    }
    return cur;
  }
  const top = parseValue(text);
  const last = emit(top.length === 1 ? top[0] : [top.map(flatParts).join(", ")], 0, "");
  lines.push(last);
  return lines.map((l) => l.replace(/^  $/, "")).filter((l, k, all) => l || k === all.length - 1);
}

/** `text` with brackets nested deeper than `depth` cut to `{…}` / `[…]` / `(…)`. */
function cutDepth(text, depth) {
  const flat = (parts, d) =>
    parts.map((p) => (typeof p === "string" ? p : p.items.length && d >= depth ? p.open + "…" + p.close : p.open + p.items.map((i) => flat(i, d + 1)).join(", ") + p.close)).join("");
  return parseValue(text).map((i) => flat(i, 0)).join(", ");
}

/**
 * One alternative in at most `width` characters: a structure (`{…}`, `Name(…)`) keeps its first
 * items whole and says `…` for the rest; otherwise the text is cut.
 */
function shorten(a, width) {
  if (a.length <= width) return a;
  const top = parseValue(a);
  const parts = top.length === 1 ? top[0] : [];
  const node = parts[parts.length - 1];
  if (node && typeof node !== "string" && parts.slice(0, -1).every((p) => typeof p === "string")) {
    const head = flatParts(parts.slice(0, -1));
    for (let n = node.items.length - 1; n >= 1; n--) {
      const s = head + node.open + node.items.slice(0, n).map(flatParts).join(", ") + ", …" + node.close;
      if (s.length <= width) return s;
    }
    const cut = cutDepth(a, 1);
    if (cut.length <= width) return cut;
  }
  return a.slice(0, width - 1) + "…";
}

/**
 * A value's lines for a hover: pretty-printed, nested levels cut until it fits in
 * `maxLines` (a hover should not fill the screen), else its first lines and how many more.
 */
function hoverValue(text, maxLines = 16) {
  let lines = prettyValue(text);
  for (let d = 3; d >= 1 && lines.length > maxLines; d--) lines = prettyValue(alternatives(text).map((a) => cutDepth(a, d)).join(" | "));
  // (several large alternatives: one line each, cut at the width)
  const alts = alternatives(text);
  if (lines.length > maxLines && alts.length > 1) {
    lines = alts.map((a, k) => (k ? "| " : "  ") + shorten(a, VALUE_WIDTH));
  }
  // (alternatives one line each that cutting made alike — they differ in what `…` hides: once)
  if (alts.length > 1 && lines.length === alts.length) {
    const kept = [...new Set(lines.map((l) => l.slice(2)))];
    if (kept.length < lines.length) lines = kept.length === 1 ? kept : kept.map((l, k) => (k ? "| " : "  ") + l);
  }
  if (lines.length > maxLines) lines = lines.slice(0, maxLines - 1).concat([`… (${lines.length - maxLines + 1} more lines)`]);
  return lines;
}

/**
 * Where an unknown comes from, in words (`site_text`'s forms): "`u` is not defined in this
 * file", "the result of `load(…)` on line 6", "argument `x` of `f`".
 */
function fromText(t) {
  let m = /^(`\w+`) \(not defined in this file\)$/.exec(t);
  if (m) return `${m[1]} is not defined in this file`;
  if (CALL_AT.test(t)) return "the result of " + callText(t);
  return t.startsWith("argument ") || t.startsWith("the ") ? t : `from ${t}`;
}

/** A call where it is: "`drop()` (line 57, …)" as "`drop()` on line 57, …". */
const CALL_AT = /^(`[^`]*`) \(([^()]*?)line (\d+)(.*)\)$/;
function callText(t) {
  return t.replace(CALL_AT, "$1 on $2line $3$4");
}

/** At most `max` bullets, the rest counted. */
function bullets(items, max) {
  return items.length > max ? items.slice(0, max - 1).concat([`and ${items.length - max + 1} more`]) : items;
}

/**
 * The headline's lines: `name = value`, the value pretty-printed (`hoverValue`); a value that
 * is one of several long alternatives continues with `| B` lines, indented. `tag`: what the
 * name is, before its value (`(parameter)`).
 */
function headLines(name, value, tag) {
  const pre = `${name} = ${tag ? tag + (value ? " " : "") : ""}`;
  if (!value) return [pre.trimEnd()];
  const lines = hoverValue(value);
  const alts = lines.length > 1 && lines[0].startsWith("  ") && lines.some((l) => l.startsWith("| "));
  if (alts) return [pre + lines[0].slice(2), ...lines.slice(1).map((l) => "  " + l)];
  return [pre + lines[0], ...lines.slice(1)];
}

/**
 * A parameter's value for any call, after `(parameter)`: its default, its annotated type
 * (`int(?)`) or both (`4 | int(?)`); the unknown argument itself (`x`, named by the parameter)
 * is its type, or `?` beside a default (`3 | ?`: 3, or whatever a call passes); empty when
 * nothing is known of it.
 */
function paramValue(text, name, type) {
  const own = type ? `${type}(?)` : "?";
  const alts = [...new Set(alternatives(text).map((a) => (a === name ? own : a)))];
  if (alts.length === 1 && alts[0] === "?") return "";
  return alts.filter((a) => a !== "?").concat(alts.includes("?") ? ["?"] : []).join(" | ");
}

/**
 * What the code needs of the name. A parameter (at its `def`, for any call: `contract`; in the
 * body, where it holds the argument of its name, what the code reads of it: `required`): tensor
 * shapes (`x.dim() == 3`) and the access paths it reads, always first, then under a test of the
 * arguments (`(only if admin)`), then on some paths. Any other name: what the program reads of
 * an input it gets on this line (`cfg = json.loads(text)`: `cfg["name"]`, the columns of
 * `pd.read_csv(…)`), else nothing (what its function needs is said at its parameters).
 */
function requirementLines(f) {
  const objs = [...(f.objects || []), ...(f.general_unknowns || [])];
  const param = f.param || objs.some((o) => o.kind === "unknown" && (o.from || []).some((t) => t.startsWith(`argument \`${f.name}\` of `)));
  const input = objs.some((o) => (o.origin && o.origin.line === f.line) || (o.from || []).some((t) => t.endsWith(`(line ${f.line})`)));
  if (!param && !input) return [];
  const reads = (f.param ? f.contract : f.required) || [];
  const rank = (r) => (r.always ? 0 : r.only_if ? 1 : 2);
  const out = (param ? f.requires || [] : []).map((r) => "`" + r + "`");
  for (const r of [...reads].sort((a, b) => rank(a) - rank(b))) {
    out.push("`" + r.path + "`" + (r.always ? "" : r.only_if ? ` (only if \`${r.only_if}\`)` : " on some paths"));
  }
  return out;
}

/**
 * What the program's own calls gave, apart from the value for any call: "`3` on line 5.", values
 * the cut makes alike once with all their lines; a parameter's: what fails in the function for
 * what they pass, the worst mark first (`✗ \`row["since"]\``), nothing when it all works.
 */
function callLines(f, marks) {
  const merged = new Map();
  for (const c of f.calls || []) {
    const t = shorten(c.text, VALUE_WIDTH);
    const m = merged.get(t) || { lines: [], check: null };
    m.lines = m.lines.concat((c.lines || []).filter((l) => !m.lines.includes(l)));
    if (c.check && (!m.check || CHECK_ORDER.indexOf(c.check.tier) < CHECK_ORDER.indexOf(m.check.tier))) m.check = { tier: c.check.tier, fails: [...new Set([...(m.check ? m.check.fails : []), ...c.check.fails])], errors: [...new Set([...(m.check ? m.check.errors || [] : []), ...(c.check.errors || [])])] };
    merged.set(t, m);
  }
  const check = (c) => {
    const mark = c && MARK[c.tier];
    if (!mark) return ".";
    marks.add(mark);
    const fails = c.fails.slice(0, 3).map((x) => "`" + x + "`").join(", ") + (c.fails.length > 3 ? ` and ${c.fails.length - 3} more` : "");
    return `: ${mark}` + (fails ? " " + fails : "") + ((c.errors || []).length ? ` (${c.errors[0]})` : "");
  };
  return [...merged].map(([t, m]) => "`" + t + "`" + (m.lines.length ? " " + onLines(m.lines) : "") + check(m.check));
}

/** "on line 5", "on lines 5, 9, 12 and 2 more" */
function onLines(ls) {
  return `on line${ls.length > 1 ? "s" : ""} ${ls.length > 3 ? `${ls.slice(0, 3).join(", ")} and ${ls.length - 3} more` : ls.join(", ")}`;
}

/** Kinds of objects that are definitions or library objects, not data the program made. */
const DEFS = ["function", "class", "module", "external", "bound", "method"];

/**
 * Where the objects the name may hold were made ("created on line 28", "one of 7 objects, all
 * created on line 28 (by different calls or loop iterations)"), and the keys and attributes none
 * of them has here ("never has key "x" here"). Nothing for a parameter at its `def` (its objects
 * are the calls'), an unknown, or one object made on the hovered line itself.
 */
function siteLines(f) {
  const listed = f.objects || [];
  const objs = listed.filter((o) => o.kind !== "unknown" && !DEFS.includes(o.kind));
  if (f.param || !objs.length) return [];
  const total = Math.max(f.objects_total || 0, listed.length) - (listed.length - objs.length);
  const made = objs.filter((o) => o.created);
  const files = [...new Set(made.map((o) => o.created.file))];
  const where = (file) => {
    const ls = [...new Set(made.filter((o) => o.created.file === file).map((o) => o.created.line))].sort((a, b) => a - b);
    return { ls, text: `${onLines(ls)}${file ? ` in ${file}` : ""}` };
  };
  const places = files.map(where);
  const out = [];
  if (total > 1) {
    const one = places.length === 1 && places[0].ls.length === 1;
    out.push(`one of ${total} objects${places.length ? `, ${one ? "all " : ""}created ${places.map((p) => p.text).join("; ")}` : ""}${one ? " (by different calls or loop iterations)" : ""}`);
  } else if (places.length === 1 && !(places[0].ls[0] === f.line && !files[0])) {
    out.push(`created ${places[0].text}`);
  }
  // (what every one of them lacks: all of them listed, no unknown among them)
  if (listed.length === objs.length && total === objs.length) {
    const common = (field) => objs.map((o) => o[field] || []).reduce((a, b) => a.filter((k) => b.includes(k)));
    const keys = common("absent");
    const attrs = common("absent_attrs").map((k) => k.replace(/^"|"$/g, ""));
    const list = (xs) => xs.slice(0, 6).join(", ") + (xs.length > 6 ? ` and ${xs.length - 6} more` : "");
    if (keys.length) out.push(`never has key${keys.length > 1 ? "s" : ""} ${list(keys)} here`);
    if (attrs.length) out.push(`never has attribute${attrs.length > 1 ? "s" : ""} ${list(attrs)} here`);
  }
  return out;
}

/**
 * How the analysis that answered was cut, for the last Note (null when it ran whole): the whole
 * program's stopped at its deadline (`analysis.cut`); a module code's document while the whole
 * program's runs or after it was cut (`module`, `whole`); a function run on its own (`fn`).
 */
function runNote(run) {
  if (!run) return null;
  const ctx = run.parent || run;
  const n = (x) => Number(x).toLocaleString("en-US");
  const secs = (ms) => (ms < 1 ? "under 1 ms" : ms < 1000 ? `${Math.round(ms)} ms` : `${Math.round(ms / 100) / 10} s`);
  const stmts = (k) => `${n(k)} statement${k === 1 ? "" : "s"}`;
  const done = (a) => (a && a.statements ? ` (after ${stmts(a.statements)})` : "");
  const whole = ctx.whole ? `the whole program's analysis was cut at its ${secs(ctx.whole.deadline_ms)} deadline${done(ctx.whole)}` : ctx.pending ? "the whole program's analysis is still running" : "the whole program's analysis took too long";
  if (run.fn) {
    const a = run.analysis || {};
    const cut = a.cut ? `; this run was cut at its own ${secs(a.deadline_ms)} deadline too, so code it had not reached is taken as unknown code` : "";
    return `analysed on demand: this function alone, for any call, in ${secs(a.ms || 0)} (${stmts(a.statements || 0)}), because ${whole}${cut}`;
  }
  if (run.module) return `from the module's own code only (its functions are analysed when you hover them), because ${whole}`;
  const a = run.analysis;
  if (a && a.cut) return `the whole program's analysis was cut at its ${secs(a.deadline_ms)} deadline${done(a)}: code it had not reached by then is taken as unknown code, so values may say less than they could`;
  return null;
}

/**
 * Markdown for one name, in the owner's format: a headline `name = value` (a parameter:
 * `name = (parameter)`, with its value for any call when something is known of it), then the
 * sections that have something: **Requirements** (what the code needs of a parameter, or of an
 * input read on this line), **In Calls** (what the program's own calls give, apart from the
 * value for any call), **Allocation Sites** (where its objects were made, keys they never have)
 * and **Notes** (what each name, mark and notation used above means; last, how the analysis
 * was cut). `facts`: the entries for (line, name), each with how its document was made
 * (`run`); `verdicts`: the accesses on that line that involve the name (those that may fail
 * are diagnostics, which the editor shows in the same hover).
 */
function renderHover(name, facts, verdicts) {
  // (a name of a function, class, module or library object: the language server says it)
  facts = facts.filter((f) => !((f.objects || []).length && f.objects.every((o) => DEFS.includes(o.kind)) && !(f.text || "").includes("(")));
  if (!facts.length) return null;
  const head = [];
  const reqs = [];
  const calls = [];
  const sites = [];
  const symbols = [];
  const origins = [];
  const changed = [];
  const marks = new Set();
  const add = (list, xs) => xs.forEach((x) => list.includes(x) || list.push(x));
  for (const f of facts) {
    const text = (f.text || "").trim() || "?";
    const objs = f.objects || [];
    const legend = (f.legend || []).filter(([, d]) => d !== "?");
    // (the unknowns of the value shown: the objects are the calls' when there are some)
    const unknown = (f.general_unknowns || objs).filter((o) => o.kind === "unknown");
    const origin = (o) => (o.origin ? `the result of \`${o.origin.text}\` on line ${o.origin.line}` : (o.from || []).length ? o.from.map(fromText).join(" or ") : null);
    // (an unknown named by itself, with what is known of it: that is its value)
    const self = legend.findIndex(([sym]) => sym === name);
    let value = text;
    if (text === name && self >= 0) {
      value = legend[self][1];
      legend.splice(self, 1);
    }
    const own = value === name && !legend.length;
    let tag = null;
    if (f.param || (own && unknown.length === 1 && (unknown[0].from || []).some((t) => t.startsWith(`argument \`${name}\` of `)))) {
      // a parameter: what any call may pass, when something is known of it
      tag = "(parameter)";
      value = paramValue(value, name, f.type);
      if (/(^| )\?$/.test(value)) add(origins, ["whatever a call passes"]);
    } else if (own && f.type) {
      value = `${f.type}(?)`;
    } else if (own && !objs.length) {
      // a number the module computes from data rowan cannot see, named by its variable
      value = "?";
      add(origins, [`tensor dims equal to it are written \`${name}\``]);
    } else if (own && unknown.length === 1) {
      value = "?";
      add(origins, [origin(unknown[0])].filter(Boolean));
    } else if (value === "?" && !legend.length) {
      add(origins, unknown.map(origin).filter(Boolean).slice(0, 1));
    } else {
      // the names the value mentions, with what is known about each (Notes keeps those the
      // hover shows); where an unknown printed as `?` comes from
      for (const [sym, desc] of legend) if (!symbols.some(([s0]) => s0 === sym)) symbols.push([sym, desc]);
      add(origins, unknown.filter((o) => !o.symbol).map(origin).filter(Boolean));
    }
    // (what unknown code may have changed of the program's objects: where their `?` and `~` come from)
    if (!tag) add(changed, objs.filter((o) => o.kind !== "unknown").flatMap((o) => (o.changed_by || []).map(callText)));
    head.push(...headLines(name, value, tag));
    add(reqs, requirementLines(f));
    // (on demand there are no calls of the program: the value is for any call)
    add(calls, callLines(f, marks));
    add(sites, siteLines(f));
  }
  // Notes: what the hover above uses, and only that — the symbols it mentions (with the
  // notation their descriptions use), the notation and marks it shows
  const body = [...head, ...reqs, ...calls, ...sites].join("\n");
  const word = (w) => new RegExp(`(^|[^\\w.])${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^\\w])`).test(body);
  const used = symbols.filter(([sym]) => sym.split(", ").some(word));
  const all = [body, ...used.map(([, d]) => d)].join("\n");
  const notes = used.map(([sym, desc]) => `\`${sym}\`: \`${desc}\``);
  changed.sort((a, b) => (a === "other calls") - (b === "other calls"));
  const by = changed.length ? `may have been changed by ${changed.slice(0, 2).join(" or ")}${changed.length > 2 ? ` (or ${changed.length - 2} other call${changed.length > 3 ? "s" : ""})` : ""}` : null;
  const unknownUsed = UNKNOWN(all) || head.some((l) => / = \?$/.test(l));
  const why = origins.slice(0, 3).concat(by && unknownUsed ? [by] : []);
  if (unknownUsed) notes.push(`\`?\`: unknown${why.length ? ` (${why.join("; ")})` : ""}`);
  notes.push(...typedNotes(all));
  for (const [test, mark, meaning] of NOTATION) if (test(all)) notes.push(`\`${mark}\`: ${meaning}${mark === '"k"~' && by && !unknownUsed ? ` (${by})` : ""}`);
  for (const m of Object.keys(MARK_TEXT)) if (marks.has(m)) notes.push(`${m}: ${MARK_TEXT[m]}`);
  const cut = runNote(facts[0].run);
  if (cut) notes.push(cut);
  const out = ["```python", ...head, "```"];
  const section = (title, items, max) => items.length && out.push("", `**${title}**`, ...bullets(items, max).map((x) => `- ${x}`));
  section("Requirements", reqs, 10);
  section("In Calls", calls, 6);
  section("Allocation Sites", sites, 4);
  section("Notes", notes, 20);
  return out.join("\n");
}

/**
 * The accesses of `name` on its line: those whose text covers the hovered word (columns in
 * UTF-16 units, as the editor counts them), else any whose text mentions the name as a word.
 */
function verdictsForWord(verdicts, name, start, end) {
  const word = new RegExp(`(^|[^A-Za-z0-9_])${name}($|[^A-Za-z0-9_])`);
  return verdicts.filter((v) => {
    const t = v.text || "";
    if (!t.includes("\n") && typeof v.col === "number") return v.col <= start && end <= v.col + t.length;
    return word.test(t);
  });
}

/** The editor severity of a finding: only accesses that may fail are diagnostics. */
function severityFor(tier) {
  switch (tier) {
    case "missing": return "error";
    case "possibly":
    case "likely-missing":
    case "datakey": return "warning";
    default: return null;
  }
}

/** A diagnostic's message: the finding in words and the calls it was reached through. */
function diagnosticMessage(v) {
  const chain = v.chain && v.chain.length ? ` (via ${v.chain.map((c) => "L" + c).join("→")})` : "";
  return `${v.message || v.tier}${chain}`;
}

// ----------------------------------------------------------------- running the engine

/** An executable's file name on this platform (`rowan.exe` on Windows). */
const EXE = (name) => (process.platform === "win32" ? name + ".exe" : name);
const BIN_RELS = ["rowan/target/release", "rowan/target/debug", "target/release", "target/debug"].map((d) => path.join(d, EXE("rowan")));

/** The binary a platform-specific package carries in `bin/` (release workflow). */
function bundledBinary() {
  const p = path.join(__dirname, "bin", EXE("rowan"));
  if (!fs.existsSync(p)) return null;
  // (a .vsix is a zip: the executable bit may not survive installation)
  try {
    fs.chmodSync(p, 0o755);
  } catch (_) {}
  return p;
}

/**
 * The engine binary: the `rowan.binaryPath` setting; else the build output under a
 * workspace folder, under any ancestor of the analysed file, or next to this extension
 * (running from the repository); else the binary bundled in `bin/`; else
 * `~/.cargo/bin/rowan` (`cargo install --path rowan`); else `rowan` on PATH (with `.exe` on
 * Windows).  Returns null when nothing exists and PATH has no `rowan`.
 */
function findBinary(config, workspaceFolders, filePath) {
  const configured = config.get("binaryPath");
  if (configured) return configured;
  const roots = [];
  for (const f of workspaceFolders || []) roots.push(f.uri.fsPath);
  if (filePath) {
    let dir = path.dirname(filePath);
    for (let i = 0; i < 12; i++) {
      roots.push(dir);
      const up = path.dirname(dir);
      if (up === dir) break;
      dir = up;
    }
  }
  roots.push(path.join(__dirname, ".."));
  for (const root of roots) {
    for (const rel of BIN_RELS) {
      const p = path.join(root, rel);
      if (fs.existsSync(p)) return p;
    }
  }
  const bundled = bundledBinary();
  if (bundled) return bundled;
  const home = process.env.HOME || process.env.USERPROFILE;
  if (home) {
    const p = path.join(home, ".cargo", "bin", EXE("rowan"));
    if (fs.existsSync(p)) return p;
  }
  for (const dir of (process.env.PATH || "").split(path.delimiter)) {
    if (dir && fs.existsSync(path.join(dir, EXE("rowan")))) return path.join(dir, EXE("rowan"));
  }
  return null;
}

/**
 * What an executable was built for, from its header: `{ platform, archs, text }` with the
 * platform and architectures as Node names them (`darwin` / `linux` / `win32`, `x64` /
 * `arm64`; a macOS universal binary has several), or null for a file that is none of
 * Mach-O, ELF or PE (a script, a truncated download).
 */
function binaryFormat(file) {
  let b;
  try {
    const fd = fs.openSync(file, "r");
    b = Buffer.alloc(4096);
    const n = fs.readSync(fd, b, 0, b.length, 0);
    fs.closeSync(fd);
    b = b.subarray(0, n);
  } catch (e) {
    return null;
  }
  if (b.length < 64) return null;
  const MACHO_CPU = { 0x01000007: "x64", 0x0100000c: "arm64" };
  const ELF_MACHINE = { 0x3e: "x64", 0xb7: "arm64" };
  const PE_MACHINE = { 0x8664: "x64", 0xaa64: "arm64" };
  const result = (platform, archs, kind) => ((archs = [...new Set(archs)]), { platform, archs, text: `${kind} ${archs.join(" + ") || "of another architecture"}` });
  if (b.readUInt32LE(0) === 0xfeedfacf) return result("darwin", [MACHO_CPU[b.readUInt32LE(4)]].filter(Boolean), "Mach-O");
  if (b.readUInt32BE(0) === 0xcafebabe) {
    // (a universal binary: one cputype per slice, big-endian)
    const n = Math.min(b.readUInt32BE(4), 8);
    const archs = [];
    for (let i = 0; i < n && 8 + 20 * i + 4 <= b.length; i++) archs.push(MACHO_CPU[b.readUInt32BE(8 + 20 * i)]);
    return result("darwin", archs.filter(Boolean), "Mach-O universal");
  }
  if (b[0] === 0x7f && b.toString("latin1", 1, 4) === "ELF") {
    const le = b[5] === 1;
    const m = le ? b.readUInt16LE(18) : b.readUInt16BE(18);
    return result("linux", [ELF_MACHINE[m]].filter(Boolean), "ELF");
  }
  if (b.toString("latin1", 0, 2) === "MZ") {
    const pe = b.readUInt32LE(0x3c);
    if (pe + 6 <= b.length && b.toString("latin1", pe, pe + 4) === "PE\0\0") return result("win32", [PE_MACHINE[b.readUInt16LE(pe + 4)]].filter(Boolean), "PE");
  }
  return null;
}

/**
 * Why `file` cannot run on this machine (`platform`, `arch` default to this process's), or
 * null when it can or its format is not known (then spawning it will say).  An x64 binary
 * runs on arm64 macOS and Windows (Rosetta, emulation).
 */
function binaryMismatch(file, platform = process.platform, arch = process.arch) {
  const f = binaryFormat(file);
  if (!f) return null;
  const emulated = arch === "arm64" && (platform === "darwin" || platform === "win32") && f.archs.includes("x64");
  if (f.platform === platform && (f.archs.includes(arch) || emulated)) return null;
  const want = `${platform}-${arch}`;
  return `the rowan engine at ${file} is built for ${f.platform}${f.archs.length ? "-" + f.archs.join("/") : ""} (${f.text}), but this machine is ${want}. Install the rowan extension for ${want} (the .vsix named rowan-${want}-…), or set rowan.binaryPath to an engine built for this machine.`;
}

/** A spawn error in words: a binary for another platform says so, not `spawn ENOEXEC`. */
function spawnError(bin, err) {
  if (err && ["ENOEXEC", "EBADARCH", "UNKNOWN", "EFTYPE"].includes(err.code)) return binaryMismatch(bin) || `the rowan engine at ${bin} cannot be run on this machine (${err.code}).`;
  if (err && err.code === "EACCES") return `the rowan engine at ${bin} is not executable (EACCES): make it executable (chmod +x) or set rowan.binaryPath.`;
  if (err && err.code === "ENOENT") return `the rowan engine at ${bin} does not exist: set rowan.binaryPath.`;
  return null;
}

/**
 * One `rowan serve` process per binary path: JSON lines over stdio, requests answered in
 * order and matched by id.  The server keeps the result of each analysed file and reuses it
 * while the file and every module it loaded are unchanged.
 */
const servers = new Map();

/**
 * Listeners for a server's events (lines without an id): `{"event":"analysed","path":P,…,
 * "result":…}`, the whole program's document of a file whose `analyse` answered early from
 * its module code (`pending`).
 */
const serverEvents = new Set();

function getServer(bin, extraArgs) {
  const key = bin + "\u0000" + extraArgs.join("\u0000");
  let srv = servers.get(key);
  if (srv && !srv.dead) return srv;
  const child = spawn(bin, ["serve", ...extraArgs], { stdio: ["pipe", "pipe", "pipe"] });
  srv = { child, pending: new Map(), nextId: 1, dead: false, buffer: "" };
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    srv.buffer += chunk;
    let nl;
    while ((nl = srv.buffer.indexOf("\n")) >= 0) {
      const line = srv.buffer.slice(0, nl);
      srv.buffer = srv.buffer.slice(nl + 1);
      if (!line.trim()) continue;
      let msg;
      try {
        msg = JSON.parse(line);
      } catch (e) {
        continue;
      }
      if (msg.event) {
        for (const f of serverEvents) f(msg);
        continue;
      }
      const waiter = srv.pending.get(msg.id);
      if (waiter) {
        srv.pending.delete(msg.id);
        waiter(msg);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", () => {});
  const fail = (err) => {
    srv.dead = true;
    const error = (err instanceof Error && spawnError(bin, err)) || "rowan server exited";
    for (const waiter of srv.pending.values()) waiter({ ok: false, error, spawn: error !== "rowan server exited" });
    srv.pending.clear();
    if (servers.get(key) === srv) servers.delete(key);
  };
  child.on("error", fail);
  child.on("exit", () => fail());
  servers.set(key, srv);
  return srv;
}

function serverRequest(srv, req, timeoutSeconds) {
  return new Promise((resolve) => {
    const id = srv.nextId++;
    const timer = setTimeout(() => {
      srv.pending.delete(id);
      // (the analysis it started would run on, holding memory: the server ends it)
      if (req.path) {
        try {
          srv.child.stdin.write(JSON.stringify({ id: srv.nextId++, cmd: "cancel", paths: [req.path] }) + "\n");
        } catch (e) {
          /* already gone */
        }
      }
      resolve({ ok: false, error: "rowan server timed out" });
    }, timeoutSeconds * 1000);
    srv.pending.set(id, (msg) => {
      clearTimeout(timer);
      resolve(msg);
    });
    try {
      srv.child.stdin.write(JSON.stringify({ id, ...req }) + "\n");
    } catch (e) {
      clearTimeout(timer);
      srv.pending.delete(id);
      resolve({ ok: false, error: String(e) });
    }
  });
}

function stopServers() {
  for (const srv of servers.values()) {
    try {
      srv.child.stdin.write(JSON.stringify({ id: 0, cmd: "shutdown" }) + "\n");
      srv.child.kill();
    } catch (e) {
      /* already gone */
    }
  }
  servers.clear();
}

/**
 * On demand (the whole program took too long): the `--facts` document of the function of `file`
 * defined over `line`, its facts marked `on_demand`; `span` its first and last lines.
 */
async function hoverServer(bin, file, line, extraArgs, timeoutSeconds) {
  const srv = getServer(bin, extraArgs);
  const msg = await serverRequest(srv, { cmd: "hover", path: file, line }, timeoutSeconds);
  if (!msg.ok) throw new Error(msg.error || "rowan server error");
  for (const f of msg.result.facts || []) f.on_demand = true;
  return { result: msg.result, span: msg.span };
}

/** Add an on-demand document's facts and verdicts to an indexed result (`indexResult`). */
function mergeResult(idx, res) {
  const more = indexResult(res, idx.run);
  for (const [k, fs_] of more.facts) idx.facts.set(k, (idx.facts.get(k) || []).concat(fs_));
  for (const [l, vs] of more.verdicts) idx.verdicts.set(l, (idx.verdicts.get(l) || []).concat(vs));
}

async function runEngineServer(bin, file, extraArgs, timeoutSeconds, changed) {
  const srv = getServer(bin, extraArgs);
  if (changed && changed.length) await serverRequest(srv, { cmd: "changed", paths: changed }, timeoutSeconds);
  const msg = await serverRequest(srv, { cmd: "analyse", path: file }, timeoutSeconds);
  if (!msg.ok) throw Object.assign(new Error(msg.error || "rowan server error"), { spawn: !!msg.spawn });
  if (msg.pending) msg.result.pending = true;
  return msg.result;
}

function runEngine(bin, file, extraArgs, timeoutSeconds) {
  return new Promise((resolve, reject) => {
    execFile(bin, [file, "--facts", ...extraArgs], { maxBuffer: 256 * 1024 * 1024, timeout: timeoutSeconds * 1000 }, (err, stdout, stderr) => {
      if (err) return reject(new Error(spawnError(bin, err) || (stderr || err.message || "").trim().slice(-500)));
      try {
        resolve(JSON.parse(stdout));
      } catch (e) {
        reject(new Error("rowan returned invalid JSON"));
      }
    });
  });
}

/**
 * Index a --facts document: facts by line then name; verdicts by line; `run`, how the analysis
 * that made it was cut (`runNote`; null when it ran whole), shared by its facts: a function run
 * on demand (`span`; `parent`: the run of the document it is merged into), a module code's
 * document (`on_demand`, while the whole program's analysis runs: `pending`, or after it was
 * cut: `whole`), a whole program's cut at its deadline.
 */
function indexResult(res, parent = null) {
  const a = res.analysis;
  const run = res.span ? { fn: true, analysis: a, parent } : res.on_demand ? { module: true, pending: !!res.pending, whole: res.whole || null } : a && a.cut ? { analysis: a } : null;
  const facts = new Map();
  // (copies: the document itself stays as the engine gave it)
  for (const f of res.facts || []) {
    const key = `${f.line}:${f.name}`;
    if (!facts.has(key)) facts.set(key, []);
    facts.get(key).push({ ...f, run });
  }
  const verdicts = new Map();
  for (const v of res.verdicts || []) {
    if (v.file) continue; // verdicts in imported modules
    if (!verdicts.has(v.line)) verdicts.set(v.line, []);
    verdicts.get(v.line).push(v);
  }
  return { facts, verdicts, run };
}

// ----------------------------------------------------------------- VS Code glue

function activate(context) {
  const results = new Map(); // fsPath -> indexed result
  const diagnostics = vscode.languages.createDiagnosticCollection("rowan");
  const output = vscode.window.createOutputChannel("rowan");
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  context.subscriptions.push(diagnostics, output, status);

  async function analyse(doc) {
    if (!doc || doc.languageId !== "python" || doc.uri.scheme !== "file") return;
    const config = vscode.workspace.getConfiguration("rowan");
    const bin = findBinary(config, vscode.workspace.workspaceFolders, doc.uri.fsPath);
    if (!bin) {
      status.text = "$(warning) rowan: engine not found";
      status.show();
      const choice = await vscode.window.showErrorMessage(
        "rowan: engine binary not found. Build it with `cargo build --release` in the rowan/ directory of the rowan repository (or `cargo install --path rowan`), or set rowan.binaryPath.",
        "Open settings"
      );
      if (choice === "Open settings") vscode.commands.executeCommand("workbench.action.openSettings", "rowan.binaryPath");
      return;
    }
    const mismatch = binaryMismatch(bin);
    if (mismatch) {
      status.text = "$(warning) rowan: engine for another platform";
      status.show();
      output.appendLine(`rowan: ${mismatch}`);
      vscode.window.showErrorMessage(`rowan: ${mismatch}`);
      return;
    }
    status.text = "$(sync~spin) rowan";
    status.show();
    try {
      const depth = config.get("valueDepth");
      const extraArgs = [...(config.get("extraArgs") || []), ...(Number.isInteger(depth) && depth >= 0 ? ["--value-depth", String(depth)] : [])];
      const timeoutSeconds = config.get("timeoutSeconds") || 60;
      let res;
      if (config.get("useServer") !== false) {
        // the saved file changed: the server drops every result that depends on it
        try {
          res = await runEngineServer(bin, doc.uri.fsPath, extraArgs, timeoutSeconds, [doc.uri.fsPath]);
        } catch (e) {
          // (a binary that cannot start: running it once more would only say so again)
          if (e.spawn) throw e;
          res = await runEngine(bin, doc.uri.fsPath, extraArgs, timeoutSeconds);
        }
      } else {
        res = await runEngine(bin, doc.uri.fsPath, extraArgs, timeoutSeconds);
      }
      show(doc, res, { bin, extraArgs, timeoutSeconds, server: config.get("useServer") !== false });
    } catch (e) {
      status.text = "$(warning) rowan failed";
      output.appendLine(`${doc.uri.fsPath}: ${e.message}`);
      vscode.window.showWarningMessage(`rowan: ${e.message.split("\n")[0]}`);
    }
  }

  /** Index a document's result for hovers and set its diagnostics (`how`: the server's options). */
  function show(doc, res, how) {
    const idx = indexResult(res);
    // (the whole program takes long: each function is analysed when hovered, once; while
    // the whole program's analysis goes on, `pending`: its result replaces this one)
    if (res.on_demand && how.server) idx.onDemand = { bin: how.bin, extraArgs: how.extraArgs, timeoutSeconds: how.timeoutSeconds, spans: [], pending: new Map() };
    idx.how = how;
    results.set(doc.uri.fsPath, idx);
    const diags = [];
    for (const [line, vs] of idx.verdicts) {
      for (const v of vs) {
        const sev = severityFor(v.tier);
        if (!sev) continue;
        const ln = Math.max(0, line - 1);
        // (one line of the access: a text over several lines is marked on its first)
        const textLen = (v.text || "").split("\n")[0].length || 1;
        const range = new vscode.Range(ln, v.col, ln, v.col + textLen);
        const d = new vscode.Diagnostic(range, diagnosticMessage(v), {
          error: vscode.DiagnosticSeverity.Error,
          warning: vscode.DiagnosticSeverity.Warning,
          information: vscode.DiagnosticSeverity.Information,
          hint: vscode.DiagnosticSeverity.Hint,
        }[sev]);
        d.source = "rowan";
        diags.push(d);
      }
    }
    // definite tensor-shape mismatches
    const seenErr = new Set();
    for (const e of res.shape_errors || []) {
      if (e.file) continue; // mismatches in imported modules
      if (seenErr.has(`${e.line}:${e.message}`)) continue;
      seenErr.add(`${e.line}:${e.message}`);
      const ln = Math.max(0, e.line - 1);
      const lineText = doc.lineAt(Math.min(ln, doc.lineCount - 1)).text;
      const start = lineText.length - lineText.trimStart().length;
      const range = new vscode.Range(ln, start, ln, Math.max(start + 1, lineText.length));
      const chain = e.chain && e.chain.length ? ` (via ${e.chain.map((c) => "L" + c).join(">")})` : "";
      const d = new vscode.Diagnostic(range, `shape mismatch: ${e.message}${chain}`, vscode.DiagnosticSeverity.Error);
      d.source = "rowan";
      diags.push(d);
    }
    diagnostics.set(doc.uri, diags);
    const mode = res.pending ? " (functions analysed when hovered; the whole program is still being analysed)" : idx.onDemand ? " (functions analysed when hovered)" : "";
    status.text = `$(check) rowan: ${diags.length} findings${mode}`;
    output.appendLine(`${new Date().toISOString()} ${doc.uri.fsPath}: ${res.facts.length} facts, ${diags.length} findings${res.pending ? " (on demand until the whole program is analysed)" : ""}`);
  }

  // (the whole program's result, after an early answer from the module code: it replaces it)
  const onEvent = (msg) => {
    const doc = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === msg.path);
    const prev = results.get(msg.path);
    // (cut at its deadline: the module code's document stays, its hovers saying so)
    if (msg.event === "cut" && prev && prev.run) {
      prev.run.pending = false;
      prev.run.whole = msg.whole;
      status.text = "$(check) rowan: functions analysed when hovered (the whole program took too long)";
      return;
    }
    if (msg.event !== "analysed" || !msg.ok) return;
    if (!doc || !prev || !prev.how) return;
    show(doc, msg.result, prev.how);
    status.show();
  };
  serverEvents.add(onEvent);
  context.subscriptions.push({ dispose: () => serverEvents.delete(onEvent) });

  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ language: "python", scheme: "file" }, {
      async provideHover(doc, position) {
        const idx = results.get(doc.uri.fsPath);
        if (!idx) return null;
        const wordRange = doc.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_]*/);
        if (!wordRange) return null;
        const name = doc.getText(wordRange);
        const line = position.line + 1;
        const od = idx.onDemand;
        if (od && !od.spans.some(([a, b]) => a <= line && line <= b)) {
          // (one request per line at a time; a line outside any function is asked once)
          if (!od.pending.has(line)) {
            od.pending.set(line, hoverServer(od.bin, doc.uri.fsPath, line, od.extraArgs, od.timeoutSeconds).then(
              (r) => {
                mergeResult(idx, r.result);
                od.spans.push(r.span);
                status.text = `$(check) rowan: ${od.spans.length} function${od.spans.length > 1 ? "s" : ""} analysed on demand`;
              },
              (e) => output.appendLine(`${doc.uri.fsPath}:${line}: ${e.message}`)
            ));
          }
          await od.pending.get(line);
        }
        const facts = idx.facts.get(`${line}:${name}`) || [];
        const verdicts = verdictsForWord(idx.verdicts.get(line) || [], name, wordRange.start.character, wordRange.end.character);
        const md = renderHover(name, facts, verdicts);
        if (!md) return null;
        const ms = new vscode.MarkdownString(md);
        ms.isTrusted = false;
        return new vscode.Hover(ms, wordRange);
      },
    }),
    vscode.commands.registerCommand("rowan.analyse", () => analyse(vscode.window.activeTextEditor && vscode.window.activeTextEditor.document)),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (vscode.workspace.getConfiguration("rowan").get("analyseOnSave")) analyse(doc);
    }),
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (vscode.workspace.getConfiguration("rowan").get("analyseOnSave")) analyse(doc);
    }),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      results.delete(doc.uri.fsPath);
      diagnostics.delete(doc.uri);
    })
  );
  if (vscode.workspace.getConfiguration("rowan").get("analyseOnSave")) {
    for (const doc of vscode.workspace.textDocuments) analyse(doc);
  }
}

function deactivate() {
  stopServers();
}

module.exports = {
  shorten, activate, deactivate, renderHover, prettyValue, hoverValue, indexResult, severityFor, verdictsForWord, diagnosticMessage, findBinary,
  binaryFormat, binaryMismatch, spawnError, runEngineServer, hoverServer, mergeResult, stopServers, serverEvents };
