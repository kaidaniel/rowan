def make_acc():
    state = {"n": 0}

    def add(k):
        state["n"] += 1
        state[k] = True

    return add, state


add, st = make_acc()
for k in ["x", "y"]:
    add(k)
st["n"]                                         #@ ok n
st["x"]                                         #@ ok x
st["z"]                                         #@ missing z

items = [{"id": 1, "v": 2}, {"id": 2}]
for it in items:
    it["id"]                                    #@ ok id
    it["v"]                                     #@ possibly v
