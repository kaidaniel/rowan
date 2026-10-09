"""Hypothesis: a class wrapping a json value turns the keys its methods read into requirements
on the value (not the ones read with `.get`).

Expectations were written from the program text before running the engine."""
import json


class Cfg:
    def __init__(self, raw):
        self.raw = raw

    def host(self):
        return self.raw["host"]

    def port(self):
        return self.raw.get("port", 80)


c = Cfg(json.loads(text_in))
h = c.host()                                 #@ input host
p = c.port()                                 #@ none port
#@schema json.loads@L19: always={host} only={host}
