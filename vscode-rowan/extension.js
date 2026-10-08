// rowan VS Code extension: runs `rowan FILE --facts` and shows the inferred facts about a
// variable on hover (keys it has, keys required of it, possible literal values) plus the
// missing-key verdicts as diagnostics.  Plain JavaScript, no build step.
"use strict";

const path = require("path");
const fs = require("fs");
const { execFile } = require("child_process");

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
  datakey: "data-derived key into a closed record", map: "map key", unknown: "unknown",
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
 * `{"a": ..., "b"?: ..., "c"~: ..., **}`: definite keys plain, keys set on some paths with
 * `?`, keys unknown code may have removed with `~`, `**` when other keys are possible (the
 * record is open).  Attribute rows: `obj(a=..., b?=..., **)`.
 */
function rowText(row, ns) {
  const present = row.present || [];
  const definite = new Set(row.definite || []);
  const likely = new Set(row.likely || []);
  const parts = present.map((k) => {
    const mark = definite.has(k) ? "" : likely.has(k) ? "~" : "?";
    return ns === "attr" ? `${displayKey(k, ns)}${mark}=...` : `${displayKey(k, ns)}${mark}: ...`;
  });
  if (!row.closed) parts.push("**");
  return ns === "attr" ? `obj(${parts.join(", ")})` : `{${parts.join(", ")}}`;
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
  if (maybe.some((k) => !likely.includes(k))) legend.push("`?` set on some paths only");
  if (maybe.some((k) => likely.includes(k))) legend.push("`~` unless unknown code removed it");
  if (!row.closed) legend.push(`\`**\` other ${label} possible`);
  if (legend.length) lines.push(`- ${legend.join("; ")}`);
  if (absent.length) lines.push(`- known absent: ${keyList(absent)}`);
  if (!row.closed) {
    if (row.sources && row.sources.length) lines.push(`- other ${label} may come from ${row.sources.join(", ")}`);
    if (row.mutators && row.mutators.length) lines.push(`- unknown code may have changed it: ${row.mutators.join(", ")}`);
    if (row.top) lines.push("- nothing is known about its contents");
    if (row.symbolic) lines.push("- plus whatever the caller passes in (summary analysis)");
  }
  return lines;
}

/**
 * Markdown for one name.  `facts`: the entries for (line, name) (concrete first);
 * `verdicts`: the verdicts on that line mentioning the name's text.
 */
function renderHover(name, facts, verdicts) {
  const out = [];
  const concrete = facts.filter((f) => !f.symbolic);
  const symbolic = facts.filter((f) => f.symbolic);
  const main = concrete.length ? concrete : symbolic;
  if (!main.length && !verdicts.length) return null;
  out.push(`**rowan** · \`${name}\``);
  for (const f of main) {
    if (f.symbolic) out.push("_inside a function analysed symbolically: facts are relative to its entry_");
    // the value as far as it is known: literals, records with their values, `...` for unknown
    const text = (f.text || "").trim();
    const withText = text.length > 0 && text !== "...";
    if (withText) out.push(`- value: \`${text}\``);
    else if ((f.values || []).includes("<parameter>")) out.push("- value: whatever the caller passes");
    const objs = f.objects || [];
    if (objs.length > 1) out.push(`- may point to ${objs.length} objects:`);
    for (const o of objs) {
      const head = objs.length > 1 ? `  - **${o.label}** (${o.kind})` : `- object **${o.label}** (${o.kind})`;
      const row = o.kind === "instance" || o.kind === "namespace" ? o.attr : o.item || o.attr;
      const ns = o.kind === "instance" || o.kind === "namespace" ? "attr" : "item";
      if (!row) {
        out.push(head);
        continue;
      }
      out.push(head);
      for (const l of describeRow(row, ns, withText)) out.push((objs.length > 1 ? "  " : "") + l);
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

function severityFor(tier) {
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

/**
 * The engine binary: the `rowan.binaryPath` setting; else the build output under a
 * workspace folder, under any ancestor of the analysed file, or next to this extension
 * (running from the repository); else `~/.cargo/bin/rowan` (`cargo install --path rowan`);
 * else `rowan` on PATH.  Returns null when nothing exists and PATH has no `rowan`.
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
      const res = await runEngine(bin, doc.uri.fsPath, config.get("extraArgs") || [], config.get("timeoutSeconds") || 60);
      const idx = indexResult(res);
      results.set(doc.uri.fsPath, idx);
      const diags = [];
      for (const [line, vs] of idx.verdicts) {
        for (const v of vs) {
          const sev = severityFor(v.tier);
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

function deactivate() {}

module.exports = { activate, deactivate, renderHover, describeRow, indexResult, severityFor, findBinary };
