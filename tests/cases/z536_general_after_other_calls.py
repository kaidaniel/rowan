# A function's value for any call (`#@general`) holds whatever the program's other functions
# did before the call (they may run in any order once the module has run), and for the calls
# the program makes while the module runs (a global rebound or changed later).


class Cache:
    def __init__(self):
        self.store = {}

    def put(self, k, v):
        self.store[k] = v

    def size(self):
        n = len(self.store)
        # (put may have filled the store of any Cache first)
        #@value n: exactly={1}
        #@general n: unknown
        return n


AUDIT = []


def log(e):
    AUDIT.append(e)


def audit_size():
    m = len(AUDIT)
    # (called before log ran; any later call: log may have run any number of times)
    #@value m: exactly={0}
    #@general m: unknown
    return m


LIMIT = 1


def limit():
    x = LIMIT
    # (the program's call sees 1; a call once the module has run sees 2)
    #@value x: exactly={1}
    #@general x: exactly={1, 2}
    return x


CFG = {"mode": "a"}


def mode():
    y = CFG.get("mode")
    # (the module deletes the key after its call)
    #@value y: exactly={"a"}
    #@general y: exactly={"a", None}
    return y


c = Cache()
c.put("u", 1)
got = c.size()
before = audit_size()
log("start")
first = limit()
LIMIT = 2
m0 = mode()
del CFG["mode"]
