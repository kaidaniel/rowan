"""A summary replayed for another object (its objects renamed to the call's) puts every fact
its run wrote: an append to a list whose length the run had as unknown still makes the
caller's known length unknown (it is one more), not the length before the call."""
#@limits tab=0


class Bus:
    def __init__(self):
        self.log = []

    def emit(self, topic):
        self.log.append({"topic": topic})


def scenario(n):
    b = Bus()
    for t in ["a", "b", "c", "d", "e", "f"]:
        b.emit(t)
    return b


b1 = scenario(1)
b2 = scenario(2)
n2 = len(b2.log)                                 #@value n2: includes={6}
