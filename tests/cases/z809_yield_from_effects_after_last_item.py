"""`yield from` a generator: its effects come with each item it gives, and those after its last
item once it ends, before the delegating generator goes on."""
log = []


def inner():
    yield 1
    log.append("inner end")


def outer():
    log.append("outer start")
    yield from inner()
    log.append("outer end")


n = 0
for v in outer():
    n = len(log)
#@value n: exactly={1}
total = len(log)
#@value total: exactly={3}
middle = log[1]
#@value middle: exactly={"inner end"}
