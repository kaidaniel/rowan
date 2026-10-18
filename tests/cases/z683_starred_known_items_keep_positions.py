# A display with a starred part whose items are known (`*pre`, a list of known length) has
# them at their places: its length and every position are known, so a loop over it sees every
# item (pydoc's `Helper.symbols`: the keys of every topic are set for sure).
pre = [p + q for p in ("b", "f") for q in ("'", '"')]
inv = {"S": ("'", "'''", *pre), "O": ("+", "-")}
symbols = {"%": "X"}
for topic, ss in inv.items():
    for s in ss:
        t = symbols.get(s, topic)
        symbols[s] = t
symbols["'"]
#@ ok '
symbols["b'"]
#@ ok b'
symbols["+"]
#@ ok +
t2 = ("'", *pre, "z")
#@value t2: exactly={("'", "b'", 'b"', "f'", 'f"', "z")}
n = len(t2)
#@value n: exactly={6}
last = t2[-1]
#@value last: exactly={"z"}
xs = (0, *range(3))
#@value xs: exactly={(0, 0, 1, 2)}
