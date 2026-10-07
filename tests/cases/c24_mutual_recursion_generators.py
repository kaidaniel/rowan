def even(n, acc):
    if n == 0:
        acc["done"] = True
        return acc
    acc["even"] = n
    return odd(n - 1, acc)


def odd(n, acc):
    if n == 0:
        return acc
    acc["odd"] = n
    return even(n - 1, acc)


r = even(k, {"start": 1})            #@row r: present={start,done,even,odd} definite={start} closed
r["start"]                           #@ ok start
r["done"]                            #@ possibly done
r["nope"]                            #@ missing nope


def rows(src):
    for x in src:
        yield {"id": x, "sq": x * x}


for rec in rows(range(3)):
    rec["id"]                        #@ ok id
    rec["cube"]                      #@ missing cube
