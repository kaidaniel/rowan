"""`@overload` stubs declare call shapes for type checkers; the function that runs is the last
`def`, and its own annotations say what a call may pass. A parameter the implementation
annotates `Any` (or leaves bare) may be anything, whatever type an overload above it names.
Before, the stub's annotation stayed on the parameter (both `def`s share the function's name):
`value` was taken for a `bytes`, so the list branch was unreachable for any call.

Expectations were written from the program text before running the engine."""
from typing import Any, overload


@overload
def describe(value: int) -> str: ...


@overload
def describe(value: bytes) -> str: ...


def describe(value: Any) -> str:
    if isinstance(value, (list, tuple)):
        for v in value:
            if isinstance(v, dict):
                tag = "table"
                #@general tag: exactly={"table"}
        return "list"
    if isinstance(value, bytes):
        return "bytes"
    return "other"


@overload
def width(x: str) -> int: ...


@overload
def width(x: int) -> int: ...


def width(x):
    if isinstance(x, list):
        seen = "list"
        #@general seen: exactly={"list"}
        return len(x)
    return 0


kind = describe([{"a": 1}])
#@value kind: exactly={"list"}
w = width([1, 2])
#@value w: exactly={2}
