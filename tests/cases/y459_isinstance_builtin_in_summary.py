"""An access under `isinstance(value, str)` happens only for strings. In a function's summary
`value` is a placeholder: narrowed by a user class it becomes a placeholder of that class (the
access replays only on its instances), but narrowed by a builtin class it stayed as it was, so
the access replayed on every object a call passed, and `value.strip` read as definitely
missing on an instance of a user class (`_pydecimal.Decimal.__new__`'s string branch, given a
Decimal). With the tabulation budget at 0 every call uses its summary."""
# (summaries are gone since: every call runs at its call site)


class D:
    pass


def conv(value):
    if isinstance(value, str):
        return value.strip()  #@ sound strip
    if isinstance(value, dict):
        return value.get("k")  #@ sound get
    if isinstance(value, D):
        return "d"
    return None


a = conv(D())  #@value a: includes={"d"}
b = conv(" x ")  #@value b: includes={"x"}
c = conv({"k": 1})
print(a, b, c)
