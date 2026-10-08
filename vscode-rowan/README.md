# rowan for VS Code

Hover over a variable in a Python file to see what rowan infers about it:
the keys / attributes its objects have (always, on some paths, known absent), whether the
record is closed or open and where other keys could come from, the keys this program
requires of an external input, possible literal values, and the verdicts on that line.
Missing-key verdicts are also shown as diagnostics (✗ error, ⚠ warning, ◌ information).

The extension runs the `rowan` binary (`rowan FILE --facts`) when a Python file is
opened or saved, and on the command **Rowan: Analyse current file**. Unsaved edits are not
analysed until you save.

## Setup

1. Build the engine: `cd rowan && cargo build --release`.
2. Install the extension from the `.vsix` (`Extensions: Install from VSIX…`), or open this
   folder in VS Code and press F5 to run it in an Extension Development Host.
3. If the workspace does not contain `rowan/target/release/rowan`, set
   `rowan.binaryPath` to the binary.

Settings: `rowan.binaryPath`, `rowan.extraArgs` (e.g. `["--no-entry"]` to skip the
entry-point pass on large files), `rowan.analyseOnSave`, `rowan.timeoutSeconds`.

`node test.js` checks the hover rendering against the engine's output.
