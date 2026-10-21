"""Every expression kind."""
base = {"a": 1, "b": 2}

# Constant kinds as keys
k = {1: "one", True: "t", None: "n", 2.5: "f", "s": "str", b"by": "bytes", (1, 2): "tuple"}
k[1]                                 #@ ok 1
k[None]                              #@ ok None
k["s"]                               #@ ok s
k[(1, 2)]                            #@ ok (1, 2)
k[3]                                 #@ missing 3
k[(2, 1)]                            #@ missing (2, 1)

# BoolOp / NamedExpr / IfExp
d = base if flag else {"c": 3}
d["a"]                               #@ possibly a
d["c"]                               #@ possibly c
d["z"]                               #@ missing z
e = base or {"fallback": 1}
e["a"]                               #@ ok a
if (m := {"w": 1}) and m["w"]:       #@ ok w
    m["w"]                           #@ ok w
m["v"]                               #@ missing v

# BinOp on dicts and strings
merged = base | {"c": 3}
merged["c"]                          #@ ok c
merged["a"]                          #@ ok a
merged["d"]                          #@ missing d
pre = "x_"
key = pre + "a"
sk = {"x_a": 1}
sk[key]                              #@ ok x_a
sk[pre + "b"]                        #@ missing x_b
sk["x_%s" % "a"]                     #@ ok x_a

# UnaryOp
neg = {-1: "m"}
neg[-1]                              #@ ok -1
neg[1]                               #@ missing 1
if not flag:
    base["n"] = 1
base["n"]                            #@ possibly n

# Lambda
get_a = lambda r: r["a"]
get_a(base)                          #@ ok a
get_a({"b": 1})                      #@ missing a
mk = lambda: {"lam": 1}
mk()["lam"]                          #@ ok lam
mk()["x"]                            #@ missing x

# Dict with spread, Set, List, Tuple
spread = {**base, "c": 3, **{"d": 4}}
spread["d"]                          #@ ok d
spread["a"]                          #@ ok a
st = {"p", "q"}
lst = [{"l": 1}, {"l": 2, "m": 3}]
tup = ({"t": 1},)
lst[0]["l"]                          #@ ok l
lst[1]["m"]                          #@ ok m
lst[0]["m"]                          #@ missing m
tup[0]["t"]                          #@ ok t
tup[0]["u"]                          #@ missing u

# Comprehensions (`ge` is never consumed: its read never runs, `ok b` was wrong)
lc = [{"i": i} for i in range(3)]
lc[0]["i"]                           #@ ok i
lc[0]["j"]                           #@ missing j
dc = {s: {"v": s} for s in ("a", "b")}
dc["a"]["v"]                         #@ ok a ; ok v
dc["c"]                              #@ missing c
sc = {r["a"] for r in [base]}        #@ ok a
ge = (r["b"] for r in [base])        #@ none b
nested = [[{"n": 1}] for _ in range(2)]
nested[0][0]["n"]                    #@ ok n
inv = {v: k for k, v in {"a": "x", "b": "y"}.items()}
inv["x"]                             #@ ok x
inv["a"]                             #@ missing a
filt = {k: v for k, v in base.items() if k != "a"}
filt["b"]                            #@ ok b
filt["a"]                            #@ missing a

# Compare / Call with star args / keywords
cmp = base["a"] < base["b"]          #@ ok a ; ok b


def f(x, y=0, *args, **kwargs):
    return {"x": x, "y": y, "n": len(args), **kwargs}


f(1)["x"]                            #@ ok x
f(1, 2, 3, 4)["n"]                   #@ ok n
f(1, z=5)["z"]                       #@ ok z
f(*[1, 2])["y"]                      #@ ok y
f(**{"x": 1, "w": 2})["w"]           #@ ok w
f(1)["w"]                            #@ missing w

# f-strings / format
i = 2
fk = {"col_2": 1, "row_x": 2}
fk[f"col_{i}"]                       #@ ok col_2
fk[f"col_{i}!"]                      #@ missing col_2!
fk["row_{}".format("x")]             #@ ok row_x
fk[f"{i}"]                           #@ missing "2"

# Attribute / Subscript / Slice / Starred
class P:
    def __init__(self):
        self.items = [{"s": 1}]


pp = P()
pp.items[0]["s"]                     #@ ok s
pp.items[0]["t"]                     #@ missing t
sl = [{"a": 1}, {"a": 2}][0:1]
sl[0]["a"]                           #@ ok a
star_lst = [*lst, {"end": 1}]
star_lst[0]["l"]                     #@ any l


# Yield / YieldFrom / Await
def gen():
    yield {"g": 1}
    yield from [{"g": 2}]


for item in gen():
    item["g"]                        #@ ok g
    item["h"]                        #@ missing h


async def coro():
    return {"co": 1}


async def main():
    res = await coro()
    res["co"]                        #@ ok co
    res["no"]                        #@ missing no
