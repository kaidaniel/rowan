"""A fake pandas: if the engine followed this instead of its model, DataFrame would be a
dict without column "a"."""


def DataFrame(data):
    return {"fake": True}
