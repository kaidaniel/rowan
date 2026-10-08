"""A context manager class whose __enter__ returns a session dict that __exit__ finalises;
nested `with`, a session passed to helpers, and a second manager wrapping the first.

Expectations were written from the program text before running the engine."""


class Session:
    def __init__(self, name):
        self.name = name
        self.closed = False

    def __enter__(self):
        self.data = {"name": self.name, "ops": []}
        return self.data

    def __exit__(self, et, ev, tb):
        self.data["closed"] = True
        self.closed = True
        return False


class Timed:
    def __init__(self, inner):
        self.inner = inner

    def __enter__(self):
        s = self.inner.__enter__()
        s["timed"] = True
        return s

    def __exit__(self, et, ev, tb):
        self.inner.__exit__(et, ev, tb)
        self.inner.data["elapsed"] = 1
        return False


def record(s, op):
    s["ops"].append({"op": op})
    s["last"] = op


sess = Session("a")
with sess as s:
    record(s, "read")
    _ = 0                                 #@row s: present={name,ops,last} definite={name,ops,last} closed
_ = 0                                     #@row s: present={name,ops,last,closed} definite={name,ops,last,closed} closed
_ = 0                                     #@row sess: present={name,closed,data} definite={name,closed,data} closed
t = Timed(Session("b"))
with t as s2:
    _ = 0                                 #@row s2: present={name,ops,timed} definite={name,ops,timed} closed
_ = 0                                     #@row s2: present={name,ops,timed,closed,elapsed} definite={name,ops,timed,closed,elapsed} closed
with Session("c") as s3, Session("d") as s4:
    record(s3, "w")
    s4["from"] = s3["name"]
_ = 0                                     #@row s3: present={name,ops,last,closed} definite={name,ops,last,closed} closed
_ = 0                                     #@row s4: present={name,ops,from,closed} definite={name,ops,from,closed} closed
op0 = s["ops"][0]                         #@row op0: present={op} definite={op} closed
s2["last"]                                #@ missing last
s["elapsed"]                              #@ missing elapsed
