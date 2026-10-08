#@engine rust
"""async def / await / async for / async with."""
import json


async def fetch(url):
    return {"url": url, "status": 200}


async def load(fh):
    return json.load(fh)


class Session:
    async def __aenter__(self):
        return self

    async def __aexit__(self, *a):
        return None

    async def get(self, u):
        return {"body": u}


async def main(urls):
    r = await fetch("x")
    r["status"]                          #@ ok status
    r["body"]                            #@ missing body
    cfg = await load(fh)
    cfg["timeout"]                       #@ input timeout
    results = []
    async for u in urls:
        results.append(await fetch(u))
    for res in results:
        res["url"]                       #@ ok url
        res["nope"]                      #@ missing nope
    async with Session() as s:
        page = await s.get("u")
        page["body"]                     #@ ok body
        page["head"]                     #@ missing head
    return r


#@schema json.load@L11: always={timeout}
