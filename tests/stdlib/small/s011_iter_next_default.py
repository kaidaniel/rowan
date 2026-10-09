# iter / next with a default: the first matching record or a fallback record; iterators exhaust


def first_admin(users):
    hit = next((u for u in users if u.get("role") == "admin"), None)
    if hit is None:
        return {"name": "nobody"}
    return hit


def head(rows):
    it = iter(rows)
    first = next(it, {"empty": True})
    second = next(it, None)
    _ = 0                                        #@row first: present={id,empty} closed
    return first, second


def role_of(u):
    return u["role"]                             #@ ok role ; missing role


if __name__ == "__main__":
    a = first_admin([{"name": "a", "role": "admin"}, {"name": "b"}])  #@row a: present={name,role} definite={name,role} closed
    b = first_admin([{"name": "b"}, {"name": "c", "role": "dev"}])     #@row b: present={name} definite={name} closed
    for u in (a, b):
        try:
            role_of(u)
        except KeyError:
            pass
    f, s = head([{"id": 1}])
    #@row f: present={id} definite={id} closed
    #@value s: exactly={None}
    f2, s2 = head([])
    #@row f2: present={empty} definite={empty} closed
    nums = iter(range(3))
    total = sum(nums)                            #@value total: exactly={3}
    left = next(nums, -1)                        #@value left: exactly={-1}
    it = iter("xy")
    x = next(it)                                 #@value x: exactly={"x"}
    y = next(it, "done")                         #@value y: exactly={"y"}
    z = next(it, "done")                         #@value z: exactly={"done"}
