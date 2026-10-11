"""An unknown value whose method user classes define is analysed as an instance of each of them
(duck typing); the parts such a method reads from it — attributes, items, elements, results of
calls on them — are not duck-typed again. A pretty printer over unknown values (it recurses
into the parts a `parts` method yields, as rich's `pretty` does) then stays small: duck-typing
every part as every class made objects for every part of every part, up to the access-path
depth."""
#@objects <= 2000


class Color:
    def __init__(self, name, number):
        self.name = name
        self.number = number

    def parts(self):
        yield "name", self.name
        yield "number", self.number


class Style:
    def __init__(self, color, bgcolor, bold, italic, underline, strike):
        self.color = color
        self.bgcolor = bgcolor
        self.bold = bold
        self.italic = italic
        self.underline = underline
        self.strike = strike

    def parts(self):
        yield "color", self.color
        yield "bgcolor", self.bgcolor
        yield "bold", self.bold
        yield "italic", self.italic
        yield "underline", self.underline
        yield "strike", self.strike


class Segment:
    def __init__(self, text, style):
        self.text = text
        self.style = style

    def parts(self):
        yield "text", self.text
        yield "style", self.style


def walk(obj, depth=0):
    if hasattr(obj, "parts"):
        children = {}
        for key, child in obj.parts():
            children[key] = walk(child, depth + 1)
        return {"kind": type(obj).__name__, "children": children}
    return {"kind": "leaf", "repr": repr(obj)}


def show(x):
    # (nothing calls show: x is an unknown value, duck-typed as Color, Style or Segment)
    node = walk(x)
#@row node: present={kind, children, repr} definite={kind} closed
    return node


seg = Segment("hi", Style(Color("red", 1), Color("blue", 4), True, False, False, True))
top = walk(seg)
#@row top: present={kind, children} definite={kind, children} closed
kind = top["kind"]
#@value kind: exactly={"Segment"}
kids = top["children"]
#@row kids: present={text, style} definite={text, style} closed
style = kids["style"]["children"]
#@row style: present={color, bgcolor, bold, italic, underline, strike} definite={color, bgcolor, bold, italic, underline, strike} closed
red = style["color"]["children"]["name"]["repr"]
#@value red: exactly={"'red'"}
