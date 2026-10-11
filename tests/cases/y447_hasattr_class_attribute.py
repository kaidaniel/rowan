"""`hasattr(obj, name)` is true for an instance whose class defines the name (a method, a class
attribute), whatever the instance's own attributes are: the true branch runs, and the code after
the `if` is reached (narrowing on the instance's attribute row alone made both branches dead)."""


class C:
    kind = "c"

    def parts(self):
        return ["a"]


class D:
    pass


def walk(obj):
    if hasattr(obj, "parts"):
        return "node"
    return "leaf"


top = walk(C())
#@value top: exactly={"node"}
other = walk(D())
#@value other: exactly={"leaf"}


def label(obj):
    if hasattr(obj, "kind"):
        return obj.kind
    return "none"


lab = label(C())
#@value lab: exactly={"c"}
after = 1
#@value after: exactly={1}
