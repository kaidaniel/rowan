# rowan for VS Code

Hover over a variable in a Python file to see what rowan infers it holds, in Python-like
notation: `1 | 2`, `{"id": 1, "role"?: "admin", **?}`, `Order(id=?)`, `list[?]` (`?` = a value
rowan does not know; `"k"?:` = key set on some paths only; `"k"~:` = unless unknown code
removed it; `**?` = other keys possible; `list[X]` = any number of elements like `X`), where
other keys could come from, the keys this program requires of an external input, and the
verdicts on that line.
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
package), `rowan.valueDepth` (levels of nested containers shown in the hover's value before they are
cut to `[…]` / `{…}` / `Name(…)`; default 1), `rowan.useServer`, `rowan.analyseOnSave`,
`rowan.timeoutSeconds`.

`node test.js` checks the hover rendering against the engine's output.
