// rowan VS Code extension: runs `rowan FILE --facts` and shows the inferred facts about a
// variable on hover (keys it has, keys required of it, possible literal values) plus the
// missing-key verdicts as diagnostics.  Plain JavaScript, no build step.
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

const TIER_SYMBOL = {
  proven: "✓", likely: "✓~", possibly: "⚠", missing: "✗", "likely-missing": "⚠~",
  input: "◌", datakey: "⚠data", map: "·map", unknown: "?",
};
const TIER_TEXT = {
  proven: "ok", likely: "ok (likely — unknown code ran in between)", possibly: "possibly missing",
  missing: "definitely missing", "likely-missing": "likely missing", input: "depends on input",
  datakey: "data-derived key into a dict whose keys are all known", map: "map key", unknown: "unknown",
};

function keyList(keys) {
  return keys.length ? keys.join(", ") : "∅";
}

/** A key as the engine prints it (Python repr) in the compact form: strings double-quoted. */
function displayKey(k, ns) {
  const m = /^'(.*)'$/.exec(k);
  if (m) return ns === "attr" ? m[1] : `"${m[1].replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
  return k;
}

/**
 * Notation (as the engine prints values): `?` an unknown value; `{"a": 1, "b"?: ?, "c"~: ?,
 * **?}` a dict with "a" always, "b" on some paths, "c" unless unknown code removed it, and
 * possibly other keys (`**?`); `Name(a=1, **?)` an object; `list[X]` a list of unknown length;
 * `…` what the printer left out.
 */
const VALUE_WIDTH = 40;
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

/**
 * Lines of a value no wider than `width` where possible: a structure that fits stays on one
 * line, a larger one has one item per line, indented two spaces per level.
 */
function prettyValue(text, width = VALUE_WIDTH) {
  const lines = [];
  function emit(parts, indent, cur) {
    // `cur`: the line being built (already indented); returns the line still open
    for (const p of parts) {
      if (typeof p === "string") {
        cur += p;
        continue;
      }
      const flat = flatParts([p]);
      if (cur.length + flat.length <= width || p.items.length === 0) {
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
  return lines;
}

function rowText(row, ns) {
  const present = row.present || [];
  const definite = new Set(row.definite || []);
  const likely = new Set(row.likely || []);
  const parts = present.map((k) => {
    const mark = definite.has(k) ? "" : likely.has(k) ? "~" : "?";
    return ns === "attr" ? `${displayKey(k, ns)}${mark}=?` : `${displayKey(k, ns)}${mark}: ?`;
  });
  if (!row.closed) parts.push("**?");
  return ns === "attr" ? `obj(${parts.join(", ")})` : `{${parts.join(", ")}}`;
}

/**
 * A source of unknown values in words: `param f.b` → whatever a caller passes as `b` to `f`;
 * `json.loads@L9` → `json.loads(…)` on line 9; `name u` → the undefined name `u`.
 */
function describeSource(src) {
  let m = /^param (.+)\.([^.]+)$/.exec(src);
  if (m) return `whatever a caller passes as \`${m[2]}\` to \`${m[1]}\``;
  m = /^name (.+)$/.exec(src);
  if (m) return `the undefined name \`${m[1]}\``;
  m = /^(.+)@L(\d+)$/.exec(src);
  if (m) return `\`${m[1].replace(/^call of /, "")}(…)\` on line ${m[2]}`;
  return `\`${src}\``;
}

/**
 * What the program learned about the keys (`ns` "item") or attributes ("attr") of an unknown
 * value: the ones it read or stored, in words; null when nothing.
 */
function learnedAbout(row, ns, viaGiven) {
  const present = row.present || [];
  const definite = new Set(row.definite || []);
  const absent = row.absent || [];
  if (!present.length && !absent.length) return null;
  const label = ns === "attr" ? "attributes" : "keys";
  const parts = [];
  const always = present.filter((k) => definite.has(k));
  const some = present.filter((k) => !definite.has(k));
  const show = (ks) => ks.map((k) => displayKey(k, ns)).join(", ");
  if (always.length) parts.push(`${show(always)} (always)`);
  if (some.length) parts.push(`${show(some)} (on some paths)`);
  if (absent.length) parts.push(`not ${show(absent)}`);
  // keys of an object it may be (an argument returned by unknown code) are not keys seen on it
  return viaGiven ? `${label} it may have: ${parts.join("; ")}` : `${label} seen on it: ${parts.join("; ")}`;
}

function describeRow(row, ns, withText) {
  const lines = [];
  const present = row.present || [];
  const definite = row.definite || [];
  const likely = row.likely || [];
  const absent = row.absent || [];
  const maybe = present.filter((k) => !definite.includes(k));
  const label = ns === "attr" ? "attributes" : "keys";
  if (!withText) lines.push("- `" + rowText(row, ns) + "`");
  const legend = [];
  const mark = ns === "attr" ? (m) => `\`a${m}=\`` : (m) => `\`"k"${m}:\``;
  if (maybe.some((k) => !likely.includes(k))) legend.push(`${mark("?")} set on some paths only`);
  if (maybe.some((k) => likely.includes(k))) legend.push(`${mark("~")} there unless unknown code removed it`);
  if (!row.closed) legend.push(`\`**?\` other ${label} possible`);
  if (legend.length) lines.push(`- ${legend.join("; ")}`);
  if (absent.length) lines.push(`- known absent: ${keyList(absent)}`);
  if (!row.closed) {
    if (row.sources && row.sources.length) lines.push(`- other ${label} may come from ${row.sources.join(", ")}`);
    if (row.mutators && row.mutators.length) lines.push(`- unknown code may have changed it: ${row.mutators.join(", ")}`);
    if (row.top) lines.push("- nothing is known about its contents");
  }
  return lines;
}

/**
 * Markdown for one name.  `facts`: the entries for (line, name);
 * `verdicts`: the verdicts on that line mentioning the name's text.
 */
function renderHover(name, facts, verdicts) {
  const out = [];
  const main = facts;
  if (!main.length && !verdicts.length) return null;
  out.push(`**rowan** · \`${name}\``);
  for (const f of main) {
    // the value as far as it is known, in the program's terms: unknowns by their symbol
    // (`a`, `a(b)`, `cfg["db"]`), else `?`
    const text = (f.text || "").trim() || "?";
    const pretty = prettyValue(text);
    if (pretty.length === 1) out.push(`- value: \`${text}\``);
    else out.push("- value:", "", "```text", ...pretty, "```", "");
    // the symbols it mentions, with what is known about each
    const legend = f.legend || [];
    for (const [sym, desc] of legend) out.push(`- \`${sym}\`: \`${desc}\``);
    // tensor shapes: what this line needs of its inputs (rowan could not decide it here)
    for (const r of f.requires || []) out.push(`- requires \`${r}\``);
    // what particular calls gave (the value above holds for any call)
    const ex = f.examples || [];
    if (ex.length) out.push(`- in calls: ${ex.slice(0, 3).map((e) => "`" + e + "`").join(", ")}${ex.length > 3 ? ", …" : ""}`);
    const qmark = /(^|[\s:=(\[,|*])\?($|[\s,)\]}|])/;
    if (qmark.test(text) || legend.some(([, d]) => qmark.test(d))) out.push("- `?`: unknown");
    const objs = f.objects || [];
    const known = objs.filter((o) => o.kind !== "unknown" && o.kind !== "ph");
    // an unknown without a symbol: where it comes from
    for (const o of objs.filter((o) => (o.kind === "unknown" || o.kind === "ph") && !o.symbol)) {
      const rows = [o.item, o.attr].filter(Boolean);
      // source names without the internal lens part (`json.load@L3·∖{'x'}`)
      const srcs = [...new Set(rows.flatMap((r) => (r.sources || []).map((x) => x.split("·")[0])))];
      if (o.origin) {
        // made by a call in another function (whose names do not apply here)
        out.push(`- \`?\` is the result of \`${o.origin.text}\` (line ${o.origin.line})`);
        continue;
      }
      const from = o.kind === "ph" ? "whatever the caller passes" : srcs.length ? srcs.map(describeSource).join("; ") : o.label;
      out.push(`- \`?\` comes from ${from}`);
    }
    // (rowan describes at most 16 objects of a name: `objects_total` counts them all)
    const total = Math.max(f.objects_total || 0, objs.length);
    const more = total - objs.length;
    if (known.length > 1 || more > 0) out.push(`- may be one of ${known.length + more} objects${more > 0 ? ` (${known.length} shown)` : ""}:`);
    for (const o of known) {
      const head = known.length > 1 ? `  - **${o.label}** (${o.kind})` : `- object **${o.label}** (${o.kind})`;
      const row = o.kind === "instance" || o.kind === "namespace" ? o.attr : o.item || o.attr;
      const ns = o.kind === "instance" || o.kind === "namespace" ? "attr" : "item";
      out.push(head);
      // the key / attribute details only for objects that have keys or attributes
      if (!row || !["dict", "instance", "namespace", "frame"].includes(o.kind)) continue;
      for (const l of describeRow(row, ns, true)) out.push((known.length > 1 ? "  " : "") + l);
    }
    const req = f.required || {};
    for (const src of Object.keys(req)) {
      const always = Object.keys(req[src]).filter((k) => req[src][k]);
      const sometimes = Object.keys(req[src]).filter((k) => !req[src][k]);
      const parts = [];
      if (always.length) parts.push(`always ${keyList(always)}`);
      if (sometimes.length) parts.push(`sometimes ${keyList(sometimes)}`);
      out.push(`- this program requires of ${src}: ${parts.join("; ")}`);
    }
  }
  if (verdicts.length) {
    out.push("");
    out.push("Accesses on this line:");
    for (const v of verdicts) {
      const chain = v.chain && v.chain.length ? ` (via ${v.chain.map((c) => "L" + c).join(">")})` : "";
      out.push(`- ${TIER_SYMBOL[v.tier] || v.tier} \`${v.text}\` ${v.ns}[${v.key}]: ${TIER_TEXT[v.tier] || v.tier}${v.note ? " — " + v.note : ""}${chain}`);
    }
  }
  return out.join("\n");
}

function severityFor(tier, ns) {
  // an attribute read from an input is a requirement (see the hover / schemas), not a finding
  if (tier === "input" && ns === "attr") return null;
  switch (tier) {
    case "missing": return "error";
    case "possibly":
    case "likely-missing":
    case "datakey": return "warning";
    case "input": return "information";
    case "likely": return "hint";
    default: return null;
  }
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
          const sev = severityFor(v.tier, v.ns);
          if (!sev) continue;
          const ln = Math.max(0, line - 1);
          const textLen = (v.text || "").length || 1;
          const range = new vscode.Range(ln, v.col, ln, v.col + textLen);
          const chain = v.chain && v.chain.length ? ` (via ${v.chain.map((c) => "L" + c).join(">")})` : "";
          const msg = `${v.ns}[${v.key}] ${TIER_TEXT[v.tier] || v.tier}${v.note ? ": " + v.note : ""}${v.providers && v.providers.length ? " ← " + v.providers.join(", ") : ""}${chain}`;
          const d = new vscode.Diagnostic(range, msg, {
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
        const verdicts = (idx.verdicts.get(line) || []).filter((v) => (v.text || "").includes(name));
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

module.exports = { activate, deactivate, renderHover, prettyValue, describeRow, indexResult, severityFor, findBinary };
