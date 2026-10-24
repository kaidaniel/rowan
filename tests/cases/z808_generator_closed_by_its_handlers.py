"""Closing a suspended generator (a loop's `break`) raises GeneratorExit at its yield: a handler
of its own that catches it runs, then raises it again, ends the generator, or yields again
(`close()` then raises RuntimeError, ignored where the collector closes it: nothing more of the
generator runs)."""
log = []


def reraises():
    try:
        yield 1
    except GeneratorExit:
        log.append("exit")
        raise
    log.append("never")


def ends():
    try:
        yield 1
    except BaseException:
        log.append("caught")
    log.append("ended")


def yields_again():
    try:
        yield 1
    except BaseException:
        log.append("again")
        yield 2
    log.append("never")


for v in reraises():
    break
n1 = len(log)
#@value n1: exactly={1}
last1 = log[-1]
#@value last1: exactly={"exit"}

for v in ends():
    break
n2 = len(log)
#@value n2: includes={3}
#@todo value n2: exactly={3}  (the path that caught GeneratorExit joins the loop's own path after the try: its end is both)
last2 = log[-1]
#@value last2: includes={"ended"}
#@todo value last2: exactly={"ended"}  (as n2)

for v in yields_again():
    break
n3 = len(log)
#@value n3: includes={4}
#@todo value n3: exactly={4}  (as n2)
last3 = log[-1]
#@value last3: includes={"again"}
#@todo value last3: exactly={"again"}  (as n2)
