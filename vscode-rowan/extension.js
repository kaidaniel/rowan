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
/** The notation of values (as the engine prints them): a test on the text, and its meaning. */
const NOTATION = [
  [(t) => /(^|[\s:=(\[,|])\?($|[\s,)\]}|])/.test(t.replace(/\*\*\?/g, "")), "`?` not known statically"],
  [(t) => /["'\w]\?[:=]/.test(t), '`"k"?:` set on some paths only'],
  [(t) => /["'\w]~[:=]/.test(t), '`"k"~:` there unless code rowan cannot see removed it'],
  [(t) => t.includes("**?"), "`**?` other keys may exist"],
  [(t) => / \| /.test(t), "`a | b` one of these, depending on the path or the call"],
  [(t) => t.includes("[*]"), "`[*]` any element"],
  [(t) => t.includes("…"), "`…` not shown"],
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
 * A value's lines for a hover: pretty-printed, nested levels cut until it fits in
 * `maxLines` (a hover should not fill the screen), else its first lines and how many more.
 */
function hoverValue(text, maxLines = 16) {
  let lines = prettyValue(text);
  for (let d = 3; d >= 1 && lines.length > maxLines; d--) lines = prettyValue(alternatives(text).map((a) => cutDepth(a, d)).join(" | "));
  // (several large alternatives: one line each, cut at the width)
  const alts = alternatives(text);
  if (lines.length > maxLines && alts.length > 1) {
    lines = alts.map((a, k) => (k ? "| " : "  ") + (a.length > VALUE_WIDTH ? a.slice(0, VALUE_WIDTH - 1) + "…" : a));
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

/** The objects of a fact rowan knows (not unknown values): where they were created. */
function objectLines(f) {
  const out = [];
  const objs = f.objects || [];
  const known = objs.filter((o) => o.kind !== "unknown");
  const total = Math.max(f.objects_total || 0, objs.length) - (objs.length - known.length);
  // (functions, classes, modules, library objects: their name in the value says it)
  const made = known.filter((o) => ["dict", "list", "tuple", "set", "frame", "instance", "namespace", "series"].includes(o.kind));
  const lines = [...new Set(made.map((o) => (/@L(\d+)$/.exec(o.label) || [])[1]).filter(Boolean))].map(Number).sort((a, b) => a - b);
  if (total > 1) out.push(`- one of ${total} objects${lines.length ? `, created on line${lines.length > 1 ? "s" : ""} ${lines.join(", ")}` : ""}`);
  else if (lines.length === 1) out.push(`- created on line ${lines[0]}`);
  const changed = [...new Set(known.flatMap((o) => o.changed_by || []))].sort((a, b) => (a === "other calls") - (b === "other calls"));
  const more = changed.length - 2;
  if (changed.length) out.push(`- may have been changed by ${changed.slice(0, 2).join(" or ")}${more > 0 ? ` (or ${more} other call${more > 1 ? "s" : ""})` : ""}`);
  const absent = [...new Set(made.flatMap((o) => o.absent || []))];
  if (absent.length) out.push(`- never has ${absent.join(", ")} (removed)`);
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
    const unknown = objs.filter((o) => o.kind === "unknown");
    const info = [];
    // an unknown named by itself (a parameter, a name defined elsewhere): where it comes from
    if (text === name && unknown.length === 1 && !(f.legend || []).length) {
      const o = unknown[0];
      info.push(o.origin ? `- not known here: the result of \`${o.origin.text}\` (line ${o.origin.line})` : `- not known here: ${(o.from || [])[0] || "it comes from outside"}`);
    } else if (text === "?" && !(f.legend || []).length) {
      const o = unknown.find((o) => o.origin);
      info.push(o ? `- not known statically: the result of \`${o.origin.text}\` (line ${o.origin.line})` : "- not known statically");
    } else {
      texts.push(text);
      out.push("```python", ...hoverValue(text), "```", "");
      // what particular calls gave (the value above holds for any call)
      const ex = f.examples || [];
      if (ex.length) info.push(`- in calls: ${codeList(ex, 3)}`);
      texts.push(...ex);
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
    // tensor shapes: what this line needs of its inputs (rowan could not decide it here)
    for (const r of f.requires || []) info.push(`- needs \`${r}\``);
    // what the code reads of it, when it comes from outside (an argument, an input)
    const req = f.required || [];
    const always = req.filter((r) => r.always).map((r) => r.path);
    const sometimes = req.filter((r) => !r.always).map((r) => r.path);
    if (always.length) info.push(`- the code reads ${codeList(always)}`);
    if (sometimes.length) info.push(`- ${always.length ? "and on some paths" : "on some paths the code reads"} ${codeList(sometimes)}`);
    for (const l of objectLines(f).concat(info)) {
      texts.push(l);
      out.push(l);
    }
  }
  // the accesses on this line, once each (the call paths they were reached by do not matter here)
  const marks = new Set();
  const seen = new Set();
  const acc = [];
  // (an input's key is in "the code reads" already)
  const reads = facts.some((f) => (f.required || []).length);
  for (const v of verdicts) {
    const mark = MARK[v.tier];
    if (!mark || (mark === "◌" && reads)) continue;
    const key = `${mark}\u0000${v.text}\u0000${v.key}`;
    if (seen.has(key)) continue;
    seen.add(key);
    marks.add(mark);
    acc.push(`- ${mark} \`${v.text}\`: ${v.message || v.tier}`);
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

const BIN_RELS = ["rowan/target/release/rowan", "rowan/target/debug/rowan", "target/release/rowan", "target/debug/rowan"];

/** The binary a platform-specific package carries in `bin/` (release workflow). */
function bundledBinary() {
  const p = path.join(__dirname, "bin", process.platform === "win32" ? "rowan.exe" : "rowan");
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
 * `~/.cargo/bin/rowan` (`cargo install --path rowan`); else `rowan` on PATH.  Returns null
 * when nothing exists and PATH has no `rowan`.
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
    const p = path.join(home, ".cargo", "bin", "rowan");
    if (fs.existsSync(p)) return p;
  }
  for (const dir of (process.env.PATH || "").split(path.delimiter)) {
    if (dir && fs.existsSync(path.join(dir, "rowan"))) return path.join(dir, "rowan");
  }
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
  const fail = () => {
    srv.dead = true;
    for (const waiter of srv.pending.values()) waiter({ ok: false, error: "rowan server exited" });
    srv.pending.clear();
    if (servers.get(key) === srv) servers.delete(key);
  };
  child.on("error", fail);
  child.on("exit", fail);
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

async function runEngineServer(bin, file, extraArgs, timeoutSeconds, changed) {
  const srv = getServer(bin, extraArgs);
  if (changed && changed.length) await serverRequest(srv, { cmd: "changed", paths: changed }, timeoutSeconds);
  const msg = await serverRequest(srv, { cmd: "analyse", path: file }, timeoutSeconds);
  if (!msg.ok) throw new Error(msg.error || "rowan server error");
  return msg.result;
}

function runEngine(bin, file, extraArgs, timeoutSeconds) {
  return new Promise((resolve, reject) => {
    execFile(bin, [file, "--facts", ...extraArgs], { maxBuffer: 256 * 1024 * 1024, timeout: timeoutSeconds * 1000 }, (err, stdout, stderr) => {
      if (err) return reject(new Error((stderr || err.message || "").trim().slice(-500)));
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
          res = await runEngine(bin, doc.uri.fsPath, extraArgs, timeoutSeconds);
        }
      } else {
        res = await runEngine(bin, doc.uri.fsPath, extraArgs, timeoutSeconds);
      }
      const idx = indexResult(res);
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
      status.text = `$(check) rowan: ${diags.length} findings`;
      output.appendLine(`${new Date().toISOString()} ${doc.uri.fsPath}: ${res.facts.length} facts, ${diags.length} findings`);
    } catch (e) {
      status.text = "$(warning) rowan failed";
      output.appendLine(`${doc.uri.fsPath}: ${e.message}`);
      vscode.window.showWarningMessage(`rowan: ${e.message.split("\n")[0]}`);
    }
  }

  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ language: "python", scheme: "file" }, {
      provideHover(doc, position) {
        const idx = results.get(doc.uri.fsPath);
        if (!idx) return null;
        const wordRange = doc.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_]*/);
        if (!wordRange) return null;
        const name = doc.getText(wordRange);
        const line = position.line + 1;
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

module.exports = { activate, deactivate, renderHover, prettyValue, hoverValue, indexResult, severityFor, verdictsForWord, diagnosticMessage, findBinary };
