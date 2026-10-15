# A dict of functions indexed by a key rowan cannot compute (sktime's `convert_dict[key](obj)`,
# the key inferred from the object): every function runs on every record, also those the
# program never gives it. What fails in one of them fails where the program picks it for
# that record (possibly), not on every path (it was `missing`: a ✗ on code that works).
import codecs


def kind_of(rec):
    return codecs.decode(rec["tag"], "rot13")


def from_a(rec):
    return {"value": rec["a"]}  #@ sound a
    #@todo ok a (the key is computed by code rowan cannot follow: from_a also runs on the record from_b is for)


def from_b(rec):
    return {"value": rec["b"]}  #@ sound b
    #@todo ok b (as above)


CONVERTERS = {"a": from_a, "b": from_b}


def convert(rec):
    return CONVERTERS[kind_of(rec)](rec)


print(convert({"tag": "n", "a": 1}))
print(convert({"tag": "o", "b": 2}))
