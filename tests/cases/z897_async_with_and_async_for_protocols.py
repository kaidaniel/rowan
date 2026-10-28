"""`async with` awaits the manager's `__aenter__` and `__aexit__`; `async for` calls `__aiter__`
once and awaits `__anext__` until it raises StopAsyncIteration. (Before, `async with` called
`__enter__` / `__exit__`, which these classes lack, so neither method ran; and `async for` over
such an iterator was taken for a TypeError, so nothing after the loop ran.)

Expectations were written from the program text before running the engine."""
import asyncio

G = 0


class Cm:
    async def __aenter__(self):
        global G
        G = G * 10 + 1
        return G

    async def __aexit__(self, *exc):
        global G
        G = G * 10 + 9
        return False


class Count:
    def __init__(self):
        self.n = 0

    def __aiter__(self):
        return self

    async def __anext__(self):
        if self.n == 2:
            raise StopAsyncIteration
        self.n += 1
        return self.n


async def main():
    async with Cm() as v:
        inside = G
        #@value inside: exactly={1}
        got = v
        #@value got: exactly={1}
    after = G
    #@value after: exactly={19}
    s = 0
    async for x in Count():
        s = s + x
    total = s
    #@value total: exactly={3}


asyncio.run(main())
