# A registry decorator past its budget (timm's `@register_model`): its code only reads the
# function's name and stores the function in a global dict and returns it. Its code is not given
# to anything that may call it, so the skipped call need not apply everything the function may do
# (timm: all of a model's building code, per registered model); the function is where the code
# put it, in the dict's items and the call's result, and runs where the program calls it from
# there (here: the effects on STATE come with that call, not with the registration).
#@limits budget=1
STATE = {"built": "none"}
SIZES = {"a": 1}
_entry = {}


def build(name):
    return part1(name) + part2(name) + part3(name)


def part1(name):
    return name + "1"


def part2(name):
    return name + "2"


def part3(name):
    return name + "3"


def register(fn):
    name = fn.__name__
    _entry[name] = fn
    return fn


@register
def model_a():
    STATE["built"] = "a"
    build("a")
    SIZES["a"] = 2
    return "A"


@register
def model_b():
    STATE["built"] = "b"
    build("b")
    SIZES["b"] = 3
    return "B"


@register
def model_c():
    STATE["built"] = "c"
    build("c")
    SIZES["c"] = 4
    return "C"


before = STATE["built"]
# (no model ran yet: the registrations only stored them)
#@value before: exactly={'none'}
made = _entry["model_b"]()
#@value made: includes={'B'}
built = STATE["built"]
#@value built: includes={'b'}
direct = model_c()
#@value direct: includes={'C'}
# (the skipped registrations walk none of the models' code: 50 visits before, 25 now)
#@visits <= 35
