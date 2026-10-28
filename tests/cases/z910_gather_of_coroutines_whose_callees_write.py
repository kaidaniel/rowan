"""`asyncio.gather` of coroutines that write nothing shared themselves but call a function that
does, around a suspension: they interleave (each runs to its `await`, then each resumes), so
each one's result is not the one running them one after the other gives. (Before, they ran one
after the other: `second` was claimed 555, Python gives 55.)

Expectations were written from the program text before running the engine."""
import asyncio

G = 0


def bump():
    global G
    G = G * 10 + 5


async def step():
    bump()
    x = G
    await asyncio.sleep(0)
    bump()
    return x


async def main():
    return await asyncio.gather(step(), step())


res = asyncio.run(main())
first = res[0]
#@value first: includes={5}
second = res[1]
#@value second: includes={55}
