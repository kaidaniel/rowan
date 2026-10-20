# A method call past its budget gives back its result summary (what the method returns for
# any call: one of the receiver's lines, a str: a position of it has no verdict). The
# receiver's attributes (lines, pos) are not the items of what it returns.
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
c = second[0]  #@ none 0
#@value c: exactly={"a","c"}
