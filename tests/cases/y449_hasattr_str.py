"""`hasattr` on a str value is decided by `str`'s attributes: its methods (`index` and
`maketrans` included) are there, any other name is not (`decode` is bytes', not str's); a branch
on it runs on one side only."""

a = hasattr("red", "parts")
#@value a: exactly={False}
b = hasattr("red", "decode")
#@value b: exactly={False}
c = hasattr("red", "index")
#@value c: exactly={True}
d = hasattr("red", "upper")
#@value d: exactly={True}


def show(v):
    if hasattr(v, "parts"):
        return "node"
    return "leaf"


s = show("red")
#@value s: exactly={"leaf"}
