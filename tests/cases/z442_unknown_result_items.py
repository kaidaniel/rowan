# A method call past its budget is code rowan does not follow: what it returns is not
# known, like data from outside (`input`). The receiver's attributes (lines, pos) are not
# the items of what it returns: `second[0]` is not "item 0 is never set".
#@limits budget=2


class Buf:
    def __init__(self):
        self.lines = ["ab", "cd"]
        self.pos = 0

    def readline(self):
        a = 1
        line = self.lines[self.pos]
        self.pos += 1
        return line


b = Buf()
first = b.readline()
second = b.readline()
c = second[0]  #@ input 0
#@todo ok 0  (the second call is past readline's budget)
