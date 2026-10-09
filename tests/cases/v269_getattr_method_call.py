"""Hypothesis: `getattr(obj, "method")()` calls the bound method on that object.

Expectations were written from the program text before running the engine."""


class Svc:
    def __init__(self):
        self.calls = {}

    def ping(self):
        self.calls["ping"] = True
        return {"pong": True}


s = Svc()
r = getattr(s, "ping")()                     #@row r: present={pong} definite={pong} closed
c = s.calls                                  #@row c: present={ping} definite={ping} closed
