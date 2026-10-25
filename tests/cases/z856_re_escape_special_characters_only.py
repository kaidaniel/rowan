"""`re.escape` escapes only the characters special in a pattern (since Python 3.7): not `,`,
`:`, `'`, `"`, `/` or `!`; space and `#` are. (Before, every character that was neither a
letter, a digit nor `_` got a backslash.)

Expectations were written from the program text before running the engine."""
import re

a = re.escape("x,y , z")
#@value a: exactly={"x,y\\ ,\\ z"}
b = re.escape("a.b/c:d")
#@value b: exactly={"a\\.b/c:d"}
c = re.escape("#1!")
#@value c: exactly={"\\#1!"}
