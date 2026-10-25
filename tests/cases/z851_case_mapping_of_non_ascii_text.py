"""Title case, capitalizing, case folding and stripping are CPython's own tables: `'ß'.title()`
is 'Ss' (its title case), not 'SS' (its upper case); `'ß'.casefold()` is 'ss'; `'中a'.title()`
upper-cases the a, which follows a character that has no case; `strip()` removes the ASCII
separators `\\x1c`-`\\x1f`, which `str.isspace` counts as space. (Before, these were computed
with Rust's upper and lower case and white space: 'SS', 'ß', '中a', and the separators kept.)

Expectations were written from the program text before running the engine."""

t = "ß".title()
#@value t: includes={"Ss"}
c = "ß".capitalize()
#@value c: includes={"Ss"}
f = "ß".casefold()
#@value f: includes={"ss"}
cjk = "中a".title()
#@value cjk: includes={"中A"}
sep = "\x1cab\x1f".strip()
#@value sep: includes={"ab"}
plain = "hello world".title()
#@value plain: exactly={"Hello World"}
