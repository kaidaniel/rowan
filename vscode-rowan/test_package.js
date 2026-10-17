// node test_package.js ROWAN-TARGET-VERSION.vsix TARGET: checks a platform-specific .vsix as
// a user gets it (the v0.1.8 install failed with `spawn ENOEXEC`: a Linux binary was packed
// for macOS). (1) unpacks it; (2) checks that the bundled engine's format is the target's
// (from its header, as the extension reads it, and with `file` where there is one), and the
// manifest's target platform; then, when TARGET is this machine, (3) starts `rowan serve`
// through the extension's own code (`findBinary`, `runEngineServer`) and (4) checks hovers
// of a small two-module program with Windows line ends.
"use strict";
const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");

const [vsix, target] = process.argv.slice(2);
if (!vsix || !target) {
  console.error("usage: node test_package.js FILE.vsix TARGET (e.g. darwin-arm64, win32-x64)");
  process.exit(2);
}
const [platform, arch] = target.split("-");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rowan-vsix-"));

// ---- (1) unpack (a .vsix is a zip; Windows' own tar reads zips, Git's does not)
const unpacked = path.join(tmp, "vsix");
fs.mkdirSync(unpacked);
if (process.platform === "win32") execFileSync(path.join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe"), ["-xf", path.resolve(vsix), "-C", unpacked]);
else execFileSync("unzip", ["-q", path.resolve(vsix), "-d", unpacked]);
const ext = path.join(unpacked, "extension");
const bin = path.join(ext, "bin", platform === "win32" ? "rowan.exe" : "rowan");
assert(fs.existsSync(bin), `no engine in the package: ${path.relative(unpacked, bin)} is missing`);
const manifest = fs.readFileSync(path.join(unpacked, "extension.vsixmanifest"), "utf8");
assert(manifest.includes(`TargetPlatform="${target}"`), `the manifest is not for ${target}: ${(/TargetPlatform="[^"]*"/.exec(manifest) || ["no TargetPlatform"])[0]}`);
for (const f of ["test.js", "test_package.js"]) assert(!fs.existsSync(path.join(ext, f)), `${f} should not be packaged`);

// ---- (2) the engine's format, as the extension reads it and as `file` says
const extension = require(path.join(ext, "extension.js"));
const fmt = extension.binaryFormat(bin);
assert(fmt, `the bundled engine is not an executable (no Mach-O / ELF / PE header): ${bin}`);
assert(fmt.platform === platform && fmt.archs.includes(arch), `the bundled engine is ${fmt.text} (${fmt.platform}), not ${target}`);
assert.strictEqual(extension.binaryMismatch(bin, platform, arch), null);
// (the message a user on another platform gets)
const other = platform === "linux" ? ["darwin", "arm64"] : ["linux", "x64"];
const msg = extension.binaryMismatch(bin, ...other);
assert(msg && msg.includes(`built for ${target}`) && msg.includes(`rowan-${other.join("-")}-`), msg);
const FILE_SAYS = {
  "darwin-arm64": /Mach-O 64-bit executable arm64/,
  "darwin-x64": /Mach-O 64-bit executable x86_64/,
  "linux-x64": /ELF 64-bit LSB .*x86-64/,
  "linux-arm64": /ELF 64-bit LSB .*(ARM aarch64|aarch64)/,
  "win32-x64": /PE32\+ executable .*x86-64/,
};
let said = null;
try {
  said = execFileSync("file", ["-b", bin]).toString().trim();
} catch (_) {
  console.log("(no `file` here: the header check above stands alone)");
}
if (said !== null) assert(FILE_SAYS[target].test(said), `file(1) says the bundled engine is "${said}", not ${target}`);
console.log(`ok: ${path.basename(vsix)} holds ${path.relative(unpacked, bin)}: ${fmt.text}${said ? ` (file: ${said})` : ""}`);

// ---- (3) + (4) on the target itself: the engine through the extension's own code, and hovers
if (`${process.platform}-${process.arch}` !== target) {
  console.log(`(not running it: this machine is ${process.platform}-${process.arch})`);
  fs.rmSync(tmp, { recursive: true, force: true });
  process.exit(0);
}
const prog = path.join(tmp, "prog");
fs.mkdirSync(prog);
const crlf = (lines) => lines.join("\r\n") + "\r\n";
fs.writeFileSync(path.join(prog, "helper.py"), crlf([
  "def make(flag):",
  '    d = {"a": 1}',
  "    if flag:",
  '        d["b"] = 2',
  "    return d",
]));
const main = path.join(prog, "main.py");
fs.writeFileSync(main, crlf([
  "from helper import make",
  "r = make(True)",
  'x = r["a"]',
  'y = r["c"]',
  "total = (x +",
  "         1)",
  "z = r[",
  '    "a"]',
]));
const src = fs.readFileSync(main, "utf8").split(/\r?\n/);
// the extension finds its own engine (no setting, no workspace, nothing on the file's path)
const found = extension.findBinary({ get: () => "" }, [], main);
assert(found && fs.realpathSync(found) === fs.realpathSync(bin), `the extension would run ${found}, not its bundled ${bin}`);

(async () => {
  try {
    const res = await extension.runEngineServer(found, main, ["--value-depth", "1"], 60, [main]);
    const idx = extension.indexResult(res);
    const hover = (line, name) => {
      const m = new RegExp(`(^|[^A-Za-z0-9_])(${name})($|[^A-Za-z0-9_])`).exec(src[line - 1]);
      const start = m ? m.index + m[1].length : 0;
      return extension.renderHover(name, extension.factsAt(idx.facts.get(`${line}:${name}`) || [], start), extension.verdictsForWord(idx.verdicts.get(line) || [], name, start, start + name.length));
    };
    const r2 = hover(2, "r");
    assert(r2 && r2.includes('```python\nr = {"a": 1, "b": 2}\n```'), `hover of r on line 2:\n${r2}`);
    const x3 = hover(3, "x");
    assert(x3 && x3.includes("```python\nx = 1\n```"), `hover of x on line 3:\n${x3}`);
    // (an access that fails is a diagnostic: the editor shows it in the same hover)
    const r4 = hover(4, "r");
    const v4 = (idx.verdicts.get(4) || []).find((v) => v.tier === "missing");
    assert(r4 && r4.includes('```python\nr = {"a": 1, "b": 2}\n```') && v4 && extension.diagnosticMessage(v4) === 'key "c" is never set', `hover of r on line 4:\n${r4}`);
    const t5 = hover(5, "total");
    assert(t5 && t5.includes("```python\ntotal = 2\n```"), `hover of total on line 5:\n${t5}`);
    // (an access over several lines, `r[` / `"a"]`: no `\r` from the Windows line ends)
    assert((res.verdicts || []).some((v) => v.line === 7 && v.tier === "proven"), JSON.stringify(res.verdicts));
    for (const v of res.verdicts || []) assert(!(v.text || "").includes("\r"), JSON.stringify(v));
    // the server answers again from its cache
    const again = await extension.runEngineServer(found, main, ["--value-depth", "1"], 60, []);
    assert.deepStrictEqual(again, res);
    console.log(`ok: rowan serve from the package answered; hovers:\n\n${r2}\n\n${r4}`);
  } finally {
    extension.stopServers();
    // (Windows keeps a running .exe locked: the stopped server may not have exited yet)
    setTimeout(() => {
      try {
        fs.rmSync(tmp, { recursive: true, force: true });
      } catch (_) {}
    }, 500);
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
