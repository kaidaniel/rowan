"""Under `redirect_stdout(buf)`, a call rowan does not run (past its budget: here every
call after the first) may print into buf: its text is not known after it, not the text
before the call. (Before, the buffer kept its text.)

Expectations were written from the program text before running the engine."""
#@limits budget=0
import io
from contextlib import redirect_stdout


def greet(who):
    print(f"hello {who}")


def captured():
    buf = io.StringIO()
    with redirect_stdout(buf):
        greet("ann")
        greet("bob")
    return buf.getvalue()


out = captured()
#@value out: includes={"hello ann\nhello bob\n"}
