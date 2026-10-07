class Lit:
    def __init__(self, key):
        self.key = key

class KeysOf:
    def __init__(self, objs, exclude):
        self.objs = objs
        self.exclude = exclude

def kv_join(a, b):
    if isinstance(a, KeysOf) and isinstance(b, KeysOf) and a.exclude == b.exclude:   #@ ok exclude ; none key
        return KeysOf(a.objs | b.objs, a.exclude)
    if isinstance(a, Lit):
        return a.key                     #@ ok key ; none objs
    return None

for i in range(20):
    kv_join(Lit(i), Lit(i))
    kv_join(KeysOf(set(), frozenset()), Lit(1))
    kv_join(KeysOf(set(), frozenset()), KeysOf(set(), frozenset()))
