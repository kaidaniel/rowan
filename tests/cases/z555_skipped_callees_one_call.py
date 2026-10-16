"""A call whose callees are several methods none of which runs (out of their budget: here
every function's) is one call that does not run, for all of them: each its own walked what
it is given again (`self.__init__(**params)` on any of a thousand instances: 667 walks)."""
#@limits budget=0
#@havoc <= 120


class Red:
    def paint(self, canvas, layers):
        canvas["red"] = layers


class Green:
    def paint(self, canvas, layers):
        canvas["green"] = layers


class Blue:
    def paint(self, canvas, layers):
        canvas["blue"] = layers


class Cyan:
    def paint(self, canvas, layers):
        canvas["cyan"] = layers


class Magenta:
    def paint(self, canvas, layers):
        canvas["magenta"] = layers


class Yellow:
    def paint(self, canvas, layers):
        canvas["yellow"] = layers


def render(brushes, layers):
    canvas = {"size": 1}
    for b in brushes:
        b.paint(canvas, layers)
#@row canvas: present={size, red, green, blue, cyan, magenta, yellow} definite={} open
# (each paint stores its key; code that does not run may change the dict otherwise too)
    return canvas


layers = [[i, i + 1, i + 2] for i in range(40)]
out = render([Red(), Green(), Blue(), Cyan(), Magenta(), Yellow()] * 4, layers)
