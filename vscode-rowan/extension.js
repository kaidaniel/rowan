// rowan VS Code extension: runs `rowan FILE --facts` and shows what rowan inferred about a
// variable on hover (its value as Python, what the code reads of it, the accesses on the
// line) and the accesses that may fail as diagnostics.  Plain JavaScript, no build step.
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

/** The mark of each kind of finding (the engine's `--facts` verdict tiers). */
const MARK = {
  proven: "✓", likely: "✓~", possibly: "⚠", missing: "✗", "likely-missing": "⚠~", input: "◌", datakey: "⚠",
};
/** What each mark means, for the hover's legend line. */
const MARK_TEXT = {
  "✓": "always works",
  "✓~": "works unless code rowan cannot see removed the key",
  "⚠": "fails on some paths",
  "✗": "fails on every path",
  "⚠~": "fails unless code rowan cannot see set the key",
  "◌": "the input must provide it",
};
/**
 * A hover worked out on demand (the whole program took too long to analyse): from the function
 * alone, for any call of it.
 */
const ON_DEMAND_LINE = "- for any call of this function, from its own analysis (the whole program took too long, so the values its calls pass are not shown)";

/** The notation of values (as the engine prints them): a test on the text, and its meaning. */
const NOTATION = [
  [(t) => /(^|[\s:=(\[,|])\?($|[\s,)\]}|])/.test(t.replace(/\*\*\?/g, "")), "`?` not known statically"],
  [(t) => /\b(int|str|bool|bytes|float|list|dict|set|tuple|frozenset)\(\?\)/.test(t), "`int(?)` an int not known statically (`str(?)` a str, …)"],
  [(t) => /["'\w]\?[:=]/.test(t), '`"k"?:` set on some paths only'],
  [(t) => /["'\w]~[:=]/.test(t), '`"k"~:` there unless code rowan cannot see removed it'],
  [(t) => t.includes("**?"), "`**?` other keys may exist"],
  [(t) => / \| |(^|\n)\| /.test(t), "`a | b` one of these, depending on the path or the call"],
  [(t) => t.includes("[*]"), "`[*]` any element"],
  [(t) => t.includes("…"), "`…` not shown"],
  [(t) => t.includes("- in calls: "), "the value holds for any call once the module has run; `in calls`: what this program's calls give"],
  [(t) => t.includes(" (only if `"), "`(only if x)` read only on calls where `x` holds of the arguments as passed"],
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

/** The legend line: what each mark and notation used in `texts` means (only those used). */
function legendLine(texts, marks) {
  const t = texts.join("\n");
  const parts = NOTATION.filter(([used]) => used(t)).map(([, meaning]) => meaning);
  for (const m of Object.keys(MARK_TEXT)) if (marks.has(m)) parts.push(`${m} ${MARK_TEXT[m]}`);
  return parts.length ? `*${parts.join(" · ")}*` : null;
}

/** `a`, `b`, `c` and 3 more */
function codeList(items, max = 6) {
  const shown = items.slice(0, max).map((x) => "`" + x + "`");
  return shown.join(", ") + (items.length > max ? ` and ${items.length - max} more` : "");
}

/**
 * What the program's own calls gave, apart from the value for any call: "- in calls: `3`
 * (line 5), `7` (lines 6, 9)" (null when there is nothing apart).
 */
function callsLine(f) {
  const calls = f.calls || [];
  if (!calls.length) return null;
  // (values the cut makes alike — they differ in what `…` hides: once, with all their lines)
  const merged = new Map();
  for (const c of calls) {
    const t = shorten(c.text, VALUE_WIDTH);
    const ls = merged.get(t) || [];
    merged.set(t, ls.concat((c.lines || []).filter((l) => !ls.includes(l))));
  }
  const lines = (ls) => (ls.length ? ` (line${ls.length > 1 ? "s" : ""} ${ls.slice(0, 3).join(", ")}${ls.length > 3 ? ", …" : ""})` : "");
  const all = [...merged];
  const shown = all.slice(0, 3).map(([t, ls]) => "`" + t + "`" + lines(ls));
  return `- in calls: ${shown.join(", ")}${all.length > 3 ? ` and ${all.length - 3} more` : ""}`;
}

/** The objects of a fact rowan knows (not unknown values): where they were created. */
function objectLines(f) {
  const out = [];
  const objs = f.objects || [];
  const known = objs.filter((o) => o.kind !== "unknown");
  const total = Math.max(f.objects_total || 0, objs.length) - (objs.length - known.length);
  // (functions, classes, modules, library objects: their name in the value says it)
  const made = known.filter((o) => ["dict", "list", "tuple", "set", "frame", "instance", "namespace", "series"].includes(o.kind));
  // (where any object the program made comes from: a parser, a model object too)
  const named = known.filter((o) => !["function", "class", "module", "external", "bound", "method"].includes(o.kind));
  const lines = [...new Set(named.map((o) => (/@L(\d+)$/.exec(o.label) || [])[1]).filter(Boolean))].map(Number).sort((a, b) => a - b);
  const where = lines.length > 1 ? `, created on lines ${lines.join(", ")}` : lines.length ? `, all created on line ${lines[0]} (by different calls or loop iterations)` : "";
  if (total > 1) out.push(`- one of ${total} objects${where}`);
  else if (lines.length === 1) out.push(`- created on line ${lines[0]}`);
  const changed = [...new Set(known.flatMap((o) => o.changed_by || []))].sort((a, b) => (a === "other calls") - (b === "other calls"));
  const more = changed.length - 2;
  if (changed.length) out.push(`- may have been changed by ${changed.slice(0, 2).join(" or ")}${more > 0 ? ` (or ${more} other call${more > 1 ? "s" : ""})` : ""}`);
  const absent = [...new Set(made.flatMap((o) => o.absent || []))];
  if (absent.length) out.push(`- never has ${absent.join(", ")} here`);
  return out;
}

/**
 * Markdown for one name: its value as Python, what the code reads of it when it comes from
 * outside, where unknowns come from, the accesses on this line, and a legend of the marks
 * used. `facts`: the entries for (line, name); `verdicts`: the accesses on that line that
 * involve the name.
 */
function renderHover(name, facts, verdicts) {
  // (a name of a function, class, module or library object: the language server says it)
  const DEFS = ["function", "class", "module", "external", "bound", "method"];
  facts = facts.filter((f) => !((f.objects || []).length && f.objects.every((o) => DEFS.includes(o.kind)) && !(f.text || "").includes("(")));
  if (!facts.length && !verdicts.length) return null;
  const out = [`**rowan** · \`${name}\``, ""];
  const texts = [];
  for (const f of facts) {
    const text = (f.text || "").trim() || "?";
    const objs = f.objects || [];
    // (the unknowns of the value shown: the objects are the calls' when there are some)
    const unknown = (f.general_unknowns || objs).filter((o) => o.kind === "unknown");
    const info = [];
    // what the program's calls gave (the value shown holds for any call): after the value
    const calls = callsLine(f);
    let valueShown = false;
    texts.push(...(f.calls || []).map((c) => shorten(c.text, VALUE_WIDTH)));
    // an unknown named by itself (a parameter, a name defined elsewhere): where it comes from
    if (text === name && !objs.length && !(f.legend || []).length) {
      // a number the module computes from data rowan cannot see, named by its variable
      info.push(`- not known statically; tensor dims equal to it are written \`${name}\``);
    } else if (text === name && unknown.length === 1 && !(f.legend || []).length) {
      const o = unknown[0];
      info.push(o.origin ? `- not known here: the result of \`${o.origin.text}\` (line ${o.origin.line})` : `- not known here: ${(o.from || [])[0] || "it comes from outside"}`);
    } else if (text === "?" && !(f.legend || []).length) {
      const o = unknown.find((o) => o.origin);
      info.push(o ? `- not known statically: the result of \`${o.origin.text}\` (line ${o.origin.line})` : "- not known statically");
    } else {
      // (the legend explains the marks shown: not those of parts the cut leaves out)
      const shown = hoverValue(text);
      valueShown = true;
      texts.push(shown.join("\n"));
      out.push("```python", ...shown, "```", "");
      // the names the value mentions, with what is known about each
      for (const [sym, desc] of f.legend || []) {
        texts.push(desc);
        info.push(`- \`${sym}\` = \`${desc}\``);
      }
      // an unknown printed as `?`: where it comes from
      for (const o of unknown.filter((o) => !o.symbol)) {
        if (o.origin) info.push(`- \`?\` is the result of \`${o.origin.text}\` (line ${o.origin.line})`);
        else if ((o.from || []).length) info.push(`- \`?\` comes from ${o.from.join(" or ")}`);
      }
    }
    if (calls) info.splice(valueShown ? 0 : 1, 0, calls);
    // (on demand: the function alone, run for any call; what the program's calls pass is not known)
    if (f.on_demand) info.splice(valueShown ? 0 : 1, 0, ON_DEMAND_LINE);
    // tensor shapes: what this line needs of its inputs (rowan could not decide it here)
    for (const r of f.requires || []) info.push(`- needs \`${r}\``);
    // what the code reads of it, when it comes from outside (an argument, an input)
    const req = f.required || [];
    const always = req.filter((r) => r.always).map((r) => r.path);
    // (reads under a test of the arguments first: they say when)
    const sometimes = req.filter((r) => !r.always).sort((a, b) => !!b.only_if - !!a.only_if);
    if (always.length) info.push(`- the code reads ${codeList(always)}`);
    // (a read under a test of the arguments: `row["role"]` (only if `admin`))
    const some = sometimes.slice(0, 6).map((r) => "`" + r.path + "`" + (r.only_if ? ` (only if \`${r.only_if}\`)` : "")).join(", ") + (sometimes.length > 6 ? ` and ${sometimes.length - 6} more` : "");
    if (sometimes.length) info.push(`- ${always.length ? "and on some paths" : "on some paths the code reads"} ${some}`);
    for (const l of objectLines(f).concat(info)) {
      texts.push(l);
      out.push(l);
    }
  }
  // the accesses on this line, once each (the call paths they were reached by do not matter
  // here); one access found for several keys alike is one line (`keys "a", "b" are there`)
  const marks = new Set();
  const groups = new Map();
  // (an input's key is in "the code reads" already)
  const reads = facts.some((f) => (f.required || []).length);
  // (one access with one mark is one line, whichever of its explanations came first)
  const seen = new Set();
  for (const v of verdicts) {
    const mark = MARK[v.tier];
    if (!mark || (mark === "◌" && reads)) continue;
    const ak = `${mark}\u0000${v.text}\u0000${v.key_str}`;
    if (seen.has(ak)) continue;
    seen.add(ak);
    const msg = v.message || v.tier;
    const keyText = typeof v.key_str === "string" ? `key ${JSON.stringify(v.key_str)}` : null;
    const shape = keyText && msg.startsWith(keyText + " ") ? msg.slice(keyText.length) : null;
    const gk = `${mark}\u0000${v.text}\u0000${shape === null ? msg : shape}`;
    if (!groups.has(gk)) groups.set(gk, { mark, text: v.text, msg, shape, keys: [] });
    const g = groups.get(gk);
    if (shape !== null && !g.keys.includes(v.key_str)) g.keys.push(v.key_str);
    marks.add(mark);
  }
  const acc = [];
  for (const g of groups.values()) {
    let msg = g.msg;
    if (g.keys.length > 1) {
      const shown = g.keys.slice(0, 6).map((k) => JSON.stringify(k)).join(", ") + (g.keys.length > 6 ? ` and ${g.keys.length - 6} more` : "");
      msg = `keys ${shown}${g.shape.replace(/^ is /, " are ")}`;
    }
    acc.push(`- ${g.mark} \`${g.text}\`: ${msg}`);
  }
  if (acc.length) out.push("", ...acc);
  const legend = legendLine(texts, marks);
  if (legend) out.push("", legend);
  return out.join("\n").replace(/\n{3,}/g, "\n\n").trim();
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
  const more = indexResult(res);
  for (const [k, fs_] of more.facts) idx.facts.set(k, (idx.facts.get(k) || []).concat(fs_));
  for (const [l, vs] of more.verdicts) idx.verdicts.set(l, (idx.verdicts.get(l) || []).concat(vs));
}

async function runEngineServer(bin, file, extraArgs, timeoutSeconds, changed) {
  const srv = getServer(bin, extraArgs);
  if (changed && changed.length) await serverRequest(srv, { cmd: "changed", paths: changed }, timeoutSeconds);
  const msg = await serverRequest(srv, { cmd: "analyse", path: file }, timeoutSeconds);
  if (!msg.ok) throw Object.assign(new Error(msg.error || "rowan server error"), { spawn: !!msg.spawn });
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

/** Index a --facts document: facts by line then name; verdicts by line. */
function indexResult(res) {
  const facts = new Map();
  for (const f of res.facts || []) {
    const key = `${f.line}:${f.name}`;
    if (!facts.has(key)) facts.set(key, []);
    facts.get(key).push(f);
  }
  const verdicts = new Map();
  for (const v of res.verdicts || []) {
    if (v.file) continue; // verdicts in imported modules
    if (!verdicts.has(v.line)) verdicts.set(v.line, []);
    verdicts.get(v.line).push(v);
  }
  return { facts, verdicts };
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
      const idx = indexResult(res);
      // (the whole program took too long: each function is analysed when hovered, once)
      if (res.on_demand && config.get("useServer") !== false) idx.onDemand = { bin, extraArgs, timeoutSeconds, spans: [], pending: new Map() };
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
      status.text = `$(check) rowan: ${diags.length} findings${idx.onDemand ? " (functions analysed when hovered)" : ""}`;
      output.appendLine(`${new Date().toISOString()} ${doc.uri.fsPath}: ${res.facts.length} facts, ${diags.length} findings`);
    } catch (e) {
      status.text = "$(warning) rowan failed";
      output.appendLine(`${doc.uri.fsPath}: ${e.message}`);
      vscode.window.showWarningMessage(`rowan: ${e.message.split("\n")[0]}`);
    }
  }

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
  binaryFormat, binaryMismatch, spawnError, runEngineServer, hoverServer, mergeResult, stopServers };
