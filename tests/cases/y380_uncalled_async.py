"""An uncalled async function: its own dict is exact, reads on awaited data are not
definitely missing.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


async def fetch(client, url):
    resp = await client.get(url)
    data = resp.json()
    item = {"url": url, "status": "ok"}
    _ = 0                                    #@row item: present={url,status} definite={url,status} closed
    data["items"]                            #@ sound items
    return item
