"""For any call, a part of a parameter annotated `list[list[int]]` given to code rowan does not
see (`g(xs[0])`) may get items of any type, and every part may be that one: iterating any part
gives ints or anything. A part given items by the function itself (`xs[1].append("n")`) gives
them in every part. (Before, the parts that unknown code was not given directly were still
claimed to hold ints only.)

Expectations were written from the program text before running the engine."""


def given(xs: list[list[int]], g):
    g(xs[0])
    for row in xs:
        for y in row:
            vf = y
            #@general vf: untyped
    return 1


def appended(xs: list[list[int]]):
    xs[1].append("n")
    for z in xs[0]:
        vz = z
        #@general vz: shows={'n'} types={int}


def put(r):
    r.append("x")


given([[1], [2]], put)
appended([[3], [4]])
