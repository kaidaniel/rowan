"""A recursive call raises what its activation raises: `walk(1)`'s call `walk(0)` raises
KeyError where `fail` holds, and `walk(1)`'s own handler catches it, so `walk(1)` returns
normally and `walk(2)` goes on to its second call, whose KeyError its handler catches too."""
import sys

fail = len(sys.argv) > 1
log = []


def walk(n):
    if n <= 0:
        if fail:
            raise KeyError(n)
        return 0
    try:
        walk(n - 1)
        walk(n - 2)
    except KeyError:
        log.append(n)
    return 1


def main():
    walk(2 if len(sys.argv) > 2 else 1)
    count = len(log)
    #@value count: includes={0, 1, 2}
    #@todo value count: exactly={0, 1, 2}  (walk(1) and walk(2) are one activation: its rounds replay the raises, the appends add up to 12)
    return count


main()
