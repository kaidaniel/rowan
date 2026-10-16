"""Hypothesis: when code between a generator's items is unknown code that may change an object
the generator itself changed (a call rowan stops following, past its budget, given `self`), the
generator sees after its yield what unknown code may do: a key surely there is then there unless
that code removed it (`likely`), not perhaps deleted (`possibly`, a false warning: argparse's
`self._current_indent` in `_dedent`, stdlib12k L6603). `self` is one of three instances.

Expectations were written from the program text before running the engine."""
#@limits budget=100


class F:
    def __init__(self):
        self._cur = 0

    def _indent(self):
        self._cur += 1

    def _dedent(self):
        # (after the yield: unknown code may have run, and may have removed it)
        self._cur -= 1
        #@ likely _cur

    def fmt(self, action):
        def colored(names):
            parts = []
            for s in names:
                if self.is_long(s):
                    parts.append(s)
            return parts
        return ", ".join(colored(action.names))

    def is_long(self, s):
        return len(s) > 2

    def _iter(self, action):
        self._indent()
        yield from action.subs()
        self._dedent()

    def add(self, action):
        n = [len(self.fmt(action)) + self._cur]
        for sub in self._iter(action):
            n.append(len(self.fmt(sub)) + self._cur)
        return n

    def add_all(self, actions):
        for a in actions:
            self.add(a)


class G(F):
    pass


class H(F):
    pass
