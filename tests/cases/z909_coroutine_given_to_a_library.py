"""A coroutine given to library code rowan does not see (an event loop's `create_task`) may run
there, at any later `await` (as a task: the loop runs it when the program suspends), or never.
(Before, it ran at the call only: `after` was claimed 7, Python gives 71.)

Expectations were written from the program text before running the engine."""
import asyncio

G = 0


async def co():
    global G
    G = G * 10 + 1
    return 5


async def main():
    global G
    t = asyncio.get_running_loop().create_task(co())
    G = 7
    before = G
    #@value before: exactly={7}
    await asyncio.sleep(0)
    after = G
    #@value after: includes={71}
    r = await t
    #@value r: includes={5}
    return r


asyncio.run(main())
