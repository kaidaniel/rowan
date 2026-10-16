# A method past its budget that returns its receiver (a builder) gives the caller the receiver:
# a call chained on its result runs on the receiver too. (With `budget=0` a function's first
# call runs, the later ones are past its budget.)
#@limits budget=0


class Query:
    def __init__(self):
        self.table = "t"

    def where(self, cond):
        self.cond = cond
        return self

    def limit(self, n):
        self.n = n
        return self


def build():
    Query().where("x").limit(1)
    q = Query()
    q.where("a > 1").limit(10)
    q.cond                                   #@ sound cond
    q.n                                      #@ sound n
    return q


build()
