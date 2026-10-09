"""Hypothesis: a dict of the instance's own bound methods built in `__init__` dispatches to the
right method on the right instance.

Expectations were written from the program text before running the engine."""


class Machine:
    def __init__(self):
        self.log = {}
        self.handlers = {"start": self.on_start, "stop": self.on_stop}

    def on_start(self):
        self.log["started"] = True

    def on_stop(self):
        self.log["stopped"] = True

    def fire(self, ev):
        self.handlers[ev]()


m = Machine()
m.fire("start")
log = m.log                                  #@row log: present={started} definite={started} closed
