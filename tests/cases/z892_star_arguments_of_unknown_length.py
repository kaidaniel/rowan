"""`f(*xs)` where the items of `xs` are not known passes some number of arguments: a library
function's result is then not what it gives for no arguments. Before, the models that read only
the arguments written took such a call for one without them: `int(*xs)` was 0, `str(*xs)` was
"", `itertools.product(*lists)` gave only the empty tuple, so a loop over its items after the
first never ran (a dispatcher trying each combination of its arguments' base classes).
Literal lists keep their items: `int(*["7"])` is 7.

Expectations were written from the program text before running the engine."""
import itertools


def convert(parts):
    number = int(*parts)
    #@general number: unknown
    text = str(*parts)
    #@general text: unknown
    return number, text


def combinations(groups):
    lists = []
    for g in groups:
        lists.append(list(g))
    combos = itertools.product(*lists)
    next(combos)  # (the first combination was tried already)
    for combo in combos:
        tried = combo
        #@general tried: unknown
    return lists


seven = int(*["7"])
#@value seven: exactly={7}
pairs = list(itertools.product(*[[1, 2], ["a"]]))
# (a list has no row of keys to check: its first item below)
first_pair = pairs[0]
#@value first_pair: exactly={(1, "a")}
n, t = convert(["12"])
#@value n: exactly={12}
#@value t: exactly={"12"}
