"""A loop over every layer of a model, each found by `getattr` with a name from data: the
value read stands for every module and every attribute read from one so far, and the next
round reads attributes of all of those. A placeholder per object read and key made objects
grow with the keys to the power of the path depth (timm's `adapt_model_from_string`: 1.2M
objects); a read on a value of many objects makes one placeholder per key instead. A library
object's attributes were named by their access paths (`Formatter.fmt.act.fmt…`), without end:
in such a read they are any value."""
#@objects <= 600
import os
from string import Formatter


class Module(Formatter):
    # (a library base: an instance may have attributes the program never set)
    pass


class Conv(Module):
    def __init__(self, c):
        self.c = c
        self.fmt = Formatter  # (a library class: its attributes are the library's)


class Block(Module):
    def __init__(self, c):
        self.conv1 = Conv(c)
        self.conv2 = Conv(c)
        self.norm = Conv(c)


class Stage(Module):
    def __init__(self, c):
        self.b1 = Block(c)
        self.b2 = Block(c)
        self.b3 = Block(c)


class Net(Module):
    def __init__(self):
        self.s1 = Stage(8)
        self.s2 = Stage(16)
        self.s3 = Stage(32)
        self.s4 = Stage(64)


def extract_layer(model, layer):
    module = model
    for l in layer.split("."):
        if hasattr(module, l):
            module = getattr(module, l)
        else:
            return module
    return module


def adapt(parent, names):
    out = []
    for n in names:
        old = extract_layer(parent, n)
        c = getattr(old, "c", None)
        old.act = getattr(old, "fmt", None)  # (kept on the module: read again next time)
#@value c: includes={8, 16, 32, 64, None}
        out.append((c, getattr(old, "stride", 1), getattr(old, "padding", 0), getattr(old, "groups", 1),
                    getattr(old, "bias", None), getattr(old, "eps", 1e-5), getattr(old, "momentum", 0.1),
                    getattr(old, "affine", True), getattr(old, "dilation", 1), getattr(old, "kernel_size", 3)))
    return out


layers = os.environ.get("LAYERS", "s1,s1.b1,s1.b1.conv1,s2.b3.norm,s4.b2.conv2").split(",")
widths = adapt(Net(), layers)
first = widths[0][0]
#@value first: includes={8, 16, 32, 64, None}
