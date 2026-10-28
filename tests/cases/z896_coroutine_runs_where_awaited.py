"""A coroutine's body runs where it is awaited, not where it is called: called and never awaited
it never runs; awaited later, its changes come then; `gather` and `wait_for` await what they
are given. (Before, rowan ran it at the call: `G` was claimed 1 right after `c = bump(1)`, and
the coroutine never awaited still added 10.)

Expectations were written from the program text before running the engine."""
import asyncio

G = 0


async def bump(n):
    global G
    G = G + n
    return G


async def main():
    c = bump(1)
    before = G
    #@value before: exactly={0}
    r = await c
    #@value r: exactly={1}
    after = G
    #@value after: exactly={1}
    never = bump(10)
    del never
    still = G
    #@value still: exactly={1}
    rs = await asyncio.gather(bump(2), bump(3))
    total = G
    #@value total: exactly={6}
    last = rs[1]
    #@value last: exactly={6}
    w = await asyncio.wait_for(bump(4), 5)
    #@value w: exactly={10}
    return r


out = asyncio.run(main())
#@value out: exactly={1}
