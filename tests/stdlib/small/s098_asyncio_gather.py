# async functions with asyncio.run / asyncio.gather: awaited results are the returned dicts

import asyncio


async def fetch(user):
    await asyncio.sleep(0)
    return {"id": user["id"], "name": user.get("name", "anon")}
#@schema param fetch.user: always={id} only={id}


async def fetch_all(users):
    results = await asyncio.gather(*(fetch(u) for u in users))
    return results


async def double(x):
    return x * 2


async def main():
    one = await fetch({"id": 1, "name": "ann", "role": "x"})
    n1 = len(one)                         #@row one: present={id,name} definite={id,name} closed
    many = await fetch_all([{"id": 2}, {"id": 3, "name": "c"}])
    second = many[1]                      #@row second: present={id,name} definite={id,name} closed
    sn = second["name"]                   #@value sn: exactly={"c"}
    fn = many[0]["name"]                  #@value fn: exactly={"anon"}
    return {"one": one, "count": len(many)}


out = asyncio.run(main())                 #@row out: present={one,count} definite={one,count} closed
cnt = out["count"]                        #@value cnt: exactly={2}
v = asyncio.run(double(4))                #@value v: exactly={8}
try:
    out["many"]                           #@ missing many
except KeyError:
    pass
