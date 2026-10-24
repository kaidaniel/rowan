"""`gen.throw(exc)` raises `exc` at the yield the generator is suspended at: its own handler
runs there, from the state it was suspended in, and may yield again (that item is what `throw`
returns); a `finally` of its own runs on the way out of the try."""
G = 0
log = []


def gen():
    global G
    try:
        G = 1
        yield 1
        G = 2
        yield 2
    except ValueError:
        seen = G
        #@value seen: includes={1}
        #@todo value seen: exactly={1}  (throw() is an unknown call: the generator may stand at any yield, and G may have changed)
        log.append("caught")
        yield 3
    finally:
        log.append("finally")


it = gen()
first = next(it)
#@value first: exactly={1}
third = it.throw(ValueError())
#@value third: includes={3}
n = len(log)
#@value n: includes={1}
#@todo value n: exactly={1}  (as seen)
it.close()
m = len(log)
#@value m: includes={2}
