"""configparser: `options(sec)` and `items(sec)` of a section the text lacks raise
NoSectionError; a value whose `%(name)s` names no option raises an InterpolationError when
read (`get`, `items`), not with `raw=True`. (Before, options and items of a missing section
gave a value not known, and a failing interpolation read as some text.)

Expectations were written from the program text before running the engine."""
import configparser

cp = configparser.ConfigParser()
cp.read_string("[s]\npath = %(nope)s/b\nk = 4\n")
try:
    o = cp.options("t")
except configparser.NoSectionError:
    o = "nosection"
o_seen = o
#@value o_seen: exactly={"nosection"}
try:
    i = cp.items("t")
except configparser.NoSectionError:
    i = "nosection"
i_seen = i
#@value i_seen: exactly={"nosection"}
try:
    g = cp.get("s", "path")
except configparser.InterpolationError:
    g = "interp"
g_seen = g
#@value g_seen: exactly={"interp"}
try:
    a = cp.items("s")
except configparser.InterpolationError:
    a = "interp"
a_seen = a
#@value a_seen: exactly={"interp"}
raw = cp.get("s", "path", raw=True)
#@value raw: exactly={"%(nope)s/b"}
