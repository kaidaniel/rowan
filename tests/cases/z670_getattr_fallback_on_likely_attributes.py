# A wrapper whose `__getattr__` delegates to the stream it wraps (click's ConsoleStream and
# _FixupStream), given to unknown code, then written to in loops (click's echo inside help
# formatting). Its attributes are there unless that unknown code removed them, so each read of
# one also took the path where it was removed: a run of `__getattr__`, whose own read of
# `self._text` took that path again (two runs per read, eight per `write`; 1.67M runs in
# click+rich). On that path `__getattr__` is a call that does not run (its result unknown, what
# its code reaches possibly changed, as for any unknown code's doing): the value is the
# attribute's or unknown, as an attribute unknown code may have replaced is on any instance.
import sys
import weakref


class Wrapper:
    def __init__(self, text, buf):
        self._text = text
        self.buffer = buf

    def __getattr__(self, name):
        return getattr(self._text, name)

    def write(self, x):
        if isinstance(x, str):
            return self._text.write(x)
        try:
            self.flush()
        except Exception:
            pass
        return self.buffer.write(x)

    def isatty(self):
        return self.buffer.isatty()


class Box:
    pass


out = Wrapper(sys.stdout, Box())
weakref.finalize(out, print)


def echo(msg, file=None):
    if file is None:
        file = out
    if msg:
        file.write(msg)
    file.flush()


def fmt(rows):
    for r in rows:
        for c in r:
            echo(c)
        echo("\n")
    if out.isatty():
        echo("tty")


def main(table):
    for t in table:
        fmt(t)
    echo("done", out)


main(sys.argv)
b = out.buffer
#@value b: unknown
n = out.write("x")
#@value n: untyped
#@tabulations <= 150
