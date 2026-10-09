"""Hypothesis: `await` of an async function's call is its return value, and `asyncio.run`
returns the coroutine's result.

Expectations were written from the program text before running the engine."""

import asyncio


async def fetch(k):
    return {"key": k, "ok": True}


async def main():
    a = await fetch("a")
    a["done"] = True
    return a


r = asyncio.run(main())                      #@row r: present={key,ok,done} definite={key,ok,done} closed
