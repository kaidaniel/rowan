# argparse keeps the default list of an `append` option: a parse without the option gives
# that very list (what the program appended to it is there), and a parse with it appends to a
# copy of it. rowan read the list's length from the last length it had anywhere (the parser's
# table held the list where the state forgot it): `n` was 1 on every path, and `m` was 1 (the
# default's items left out of the copy). Expectations written from the program text and
# argparse's documented behaviour.
import argparse
import os


def build():
    p = argparse.ArgumentParser()
    p.add_argument("--only", action="append", default=[])
    return p


flag = os.environ.get("S131_FLAG") == "1"
p = build()
if flag:
    a = p.parse_args([])
    a.only.append("x")
b = p.parse_args([])
n = len(b.only)
#@value n: exactly={0, 1}
c = p.parse_args(["--only", "y"])
m = len(c.only)
#@value m: includes={1, 2}
q = build().parse_args(["--only", "y", "--only", "z"])
k = len(q.only)
#@value k: exactly={2}
