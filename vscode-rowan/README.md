# rowan for VS Code

Hover over a variable in a Python file to see what rowan infers about it: the keys /
attributes its objects have, written as `{"id": ..., "role"?: ..., **}` (`?` = on some
paths only, `~` = unless unknown code removed it, `**` = other keys possible, i.e. the record
is open; `obj(a=..., b?=...)` for attributes), where other keys could come from, the keys
this program requires of an external input, possible literal values, and the verdicts on
that line.
Missing-key verdicts are also shown as diagnostics (✗ error, ⚠ warning, ◌ information).

The extension keeps one `rowan serve` process per binary and asks it to analyse a Python
file when it is opened or saved, and on the command **Rowan: Analyse current file**; the
server reuses a result while the file and every module it imports are unchanged. If the
server cannot be started it runs `rowan FILE --facts` instead. Unsaved edits are not
analysed until you save.

## Setup

1. Build the engine: `cd rowan && cargo build --release`.
2. Install the extension from the `.vsix` (`Extensions: Install from VSIX…`), or open this
   folder in VS Code and press F5 to run it in an Extension Development Host.
3. The extension finds the binary under a workspace folder, under any ancestor of the
   file being analysed, or next to the extension when run from the repository, then in
   `~/.cargo/bin` (`cargo install --path rowan` puts it there, for use from any
   workspace), then on PATH. Otherwise set `rowan.binaryPath` to the binary.

Settings: `rowan.binaryPath`, `rowan.extraArgs` (e.g. `["--no-entry"]` to skip the
entry-point pass on large files, or `["--follow", "mylib"]` to analyse an installed
package), `rowan.useServer`, `rowan.analyseOnSave`, `rowan.timeoutSeconds`.

`node test.js` checks the hover rendering against the engine's output.
