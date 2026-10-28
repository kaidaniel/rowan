"""A task runs when the code that made it gives way: at a later `await` (up to its own first
`await`, then on), or, never awaited, before `asyncio.run` ends. (Before, rowan ran the
coroutine whole when the task was made: `now` was claimed 1 and `mid` 123.)

Expectations were written from the program text before running the engine."""
import asyncio

G = 0


async def step():
    global G
    G = G * 10 + 1


async def two_steps():
    global G
    G = G * 10 + 2
    await asyncio.sleep(0)
    G = G * 10 + 3


async def main():
    t = asyncio.create_task(step())
    now = G
    #@value now: includes={0}
    await asyncio.sleep(0)
    later = G
    #@value later: includes={1}
    await t
    done = G
    #@value done: includes={1}
    u = asyncio.create_task(two_steps())
    await asyncio.sleep(0)
    mid = G
    #@value mid: includes={12}
    await u
    end = G
    #@value end: includes={123}


async def leave_one():
    asyncio.create_task(step())


asyncio.run(main())
asyncio.run(leave_one())
fin = G
#@value fin: includes={1231}
