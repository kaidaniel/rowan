"""Unknown code given a compiled pattern, a match, a path or a frozenset cannot change them
(they have no attributes to set, no members to add): what the program knows of them stays
known.

Expectations were written from the program text before running the engine."""
import os
import pathlib
import re
import reprlib

peek = getattr(reprlib, os.environ.get("Z523_FN", "repr"))

pat = re.compile(r"(?P<word>[a-z]+)-(?P<num>\d+)")
m = pat.match("abc-12")
p = pathlib.PurePosixPath("/tmp/data.csv")
fs = frozenset({"a", "b"})
peek([pat, m, p, fs])
src = pat.pattern  #@value src: exactly={"(?P<word>[a-z]+)-(?P<num>\\d+)"}
w = m.group("word")  #@value w: exactly={abc}
suffix = p.suffix  #@value suffix: exactly={.csv}
has_c = "c" in fs  #@value has_c: exactly={False}
