# The object a call makes at a summary site (past its objects: the nested unrolled loops) that a
# raise leaves half-built, held by nothing, is garbage: the summary keeps what every completed
# call stored.
class E:
    pass
def mk2(s):
    self = object.__new__(E)
    self.a = 1
    if s == 3:
        raise ValueError("half-built")
    self.b = 2
    return self
for i in range(16):
    for j in range(16):
        for k in range(2):
            z = mk2(i + 10)
w = mk2(5)
#@row w: present={a,b} definite={a,b} closed
try:
    mk2(3)
except ValueError:
    pass
#@row w: present={a,b} definite={a,b} closed
v = mk2(4)
#@row v: present={a,b} definite={a,b} closed
v.b
#@ ok b
