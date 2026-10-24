"""A `map` over two lists the function appends to before taking the items gives the items of
the lists as they are then, in the run for any call of the function as in its call's run."""


def f():
    a = [1, 2]
    b = [3, 4]
    m = map(lambda x, y: x + y, a, b)
    a.append(5)
    b.append(6)
    n = len(list(m))
    #@general n: exactly={3}
    #@value n: exactly={3}
    return n


f()
