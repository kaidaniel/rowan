# A function that only another uncalled function calls gets its own run with open parameters,
# whichever of the two runs first: the program never calls either, so the caller's arguments
# are only one way it may be called (here they never take the `flag` branch).


def caller():
    return helper({"a": 1}, False)


def helper(d, flag):
    if flag:
        return d["b"]  #@ input b
    return d["a"]
    #@schema param helper.d: sometimes={a, b} only={a, b}


def helper2(d, flag):
    if flag:
        return d["b"]  #@ input b
    return d["a"]
    #@schema param helper2.d: sometimes={a, b} only={a, b}


def caller2():
    return helper2({"a": 1}, False)
