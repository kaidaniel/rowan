"""A closure run for any call reads its enclosing function's variables (cells) as they may be
after any calls: unknown where the run does not know them, never "nothing". Before, a cell the
run's state did not hold read as no value at all: a lookup in it raised nothing, so the handler
of its KeyError never ran and the code after the `try` was taken for unreachable (a dispatch
table filled by a `register` closure, looked up by the dispatching closure).

Expectations were written from the program text before running the engine."""


def generic(fallback):
    table = {}

    def register(kind):
        def put(f):
            table[kind] = f
            return f
        return put

    def call(subject):
        try:
            f = table[type(subject)]
        except KeyError:
            f = fallback
        chosen = f
        #@general chosen: unknown  # (a registered function, or the fallback)
        return chosen(subject)

    call.register = register
    return call


def lookup_factory():
    seen = {}

    def lookup(key):
        try:
            got = seen[key]
        except KeyError:
            got = "absent"
        out = got
        #@general out: includes={"absent"}
        return out

    def remember(key, value):
        seen[key] = value

    return lookup, remember


def memo_factory():
    memo = {}

    def recall(key):
        try:
            return memo[key]
        except KeyError:
            return "absent"

    def note(key, value):
        memo[key] = value

    return recall, note


# (the program's own calls: these closures run where they are called, from what the program
# stored; `lookup_factory`'s, which nothing calls, run for any call)
recall, note = memo_factory()
first = recall("a")
#@value first: exactly={"absent"}  # (nothing noted yet: the dict is empty)
note("a", 1)
second = recall("a")
#@value second: exactly={1}
