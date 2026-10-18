# A wrapper whose `__getattr__` reads its own wrapped stream (click's ConsoleStream). Once
# unknown code had the wrapper, its `_inner` is there unless that code removed it, so every
# read of `self._inner` runs `__getattr__("_inner")` for the path where it was removed, which
# reads `self._inner` again. That call is the running one's (same place, receiver and name)
# however the chain of call contexts grew: it used to nest 40 deep.
import weakref


class Inner:
    def __init__(self):
        self.lines = []

    def write(self, text):
        self.lines.append(text)
        return len(text)

    def flush(self):
        return None


class Wrapper:
    def __init__(self, inner):
        self._inner = inner

    def __getattr__(self, name):
        return getattr(self._inner, name)

    def write(self, text):
        self.flush()
        return self._inner.write(text)


def echo(stream, msg):
    stream.write(msg)
    stream.flush()


w = Wrapper(Inner())
weakref.finalize(w, print)
echo(w, "a")
echo(w, "bc")
n = w.write("def")  # (was types={int}: the unknown code that had w may have put anything under _inner)
#@value n: untyped
#@work <= 250
