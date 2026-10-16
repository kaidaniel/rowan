# Unknown code given an enum member cannot change its `name` or `value` (CPython raises
# AttributeError for both); attributes of its own it may.
import enum


class Mode(enum.Enum):
    FAST = "fast"
    SLOW = "slow"


CURRENT = {"mode": Mode.FAST}


def touch(lib):
    lib.poke(Mode.FAST)
    lib.poke(CURRENT)


def fast_value():
    v = Mode.FAST.value
    n = Mode.FAST.name
    # (touch may have run: it gives the member to unknown code)
    #@value v: exactly={"fast"}
    #@general v: exactly={"fast"}
    #@general n: exactly={"FAST"}
    return v


def current():
    m = CURRENT["mode"]
    # (unknown code may have stored anything under "mode")
    #@general m: unknown
    return m


if __name__ == "__main__":
    assert fast_value() == "fast"
    assert current() is Mode.FAST
