# A known text added before or after a value that is some texts or an unknown (here `s`
# after a loop of unknown count: "ab", its repr, the repr of that, ...): the result is any
# text, not only the known texts with the affix.
import sys

n = len(sys.argv)
s = "ab"
for _i in range(n - 1):
    s = f"{s!r}"
    s = s.zfill(4)
t = "" + s                                   #@value t: unknown
u = "<" + s                                  #@value u: unknown
v = s + ">"                                  #@value v: unknown
