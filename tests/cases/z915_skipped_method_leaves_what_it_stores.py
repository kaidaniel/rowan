"""A method call that does not run (`budget=0`: each function's first call runs, the later ones
do not) leaves on its receiver what its own code stores there where the text says the value: a
literal, `+= 1` of an int an int. A store of a parameter leaves it unknown, as before; so does
`+= 1` of a value that may be no int (a float stays a float). (Before, every attribute such a
call stored was unknown after it.)

Expectations were written from the program text before running the engine."""
#@limits budget=0


class Job:
    def __init__(self):
        self.state = "new"
        self.tries = 0
        self.ratio = 0.5

    def start(self):
        self.state = "running"
        self.tries += 1
        self.ratio += 1

    def stop(self, why):
        self.state = why


# (the first call of each runs; the calls on `j` after do not)
w = Job()
w.start()
w.stop("warm")
j = Job()
j.state = "new"
j.tries = 0
j.ratio = 0.5
j.start()
s1 = j.state
#@value s1: exactly={'running'}
t1 = j.tries
#@value t1: types={int}
r1 = j.ratio
#@value r1: untyped
j.stop("halt")
s2 = j.state
#@value s2: unknown
