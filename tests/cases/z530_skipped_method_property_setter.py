"""A method past its budget that assigns a property with a setter on its receiver
(`self.plain = …`) runs the setter: nothing is stored on the instance under the property's
name, so reading the property still works; the setter's own stores may have changed.
(Each method's second call is past the budget.)

Expectations were written from the program text before running the engine."""
#@limits budget=2


class Text:
    def __init__(self, s):
        self._text = s

    @property
    def plain(self):
        return self._text

    @plain.setter
    def plain(self, s):
        self._text = s

    def reset(self, s):
        a = 1
        if not s:
            return
        self.plain = s


t = Text("a")
t.reset("b")
t.reset("c")
p = t.plain  #@ ok plain
raw = t._text  #@ ok _text
