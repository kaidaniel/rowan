# A generator run whole for a consumer that runs unknown code between its items runs again when
# that code changed what it read: here `con` and ENV (both given to `hook`, unknown code; ENV is
# also what `con` reaches through its class's `environ`). The first run read only `con.height`;
# the run again, with `con` changed, may take the other branch and read ENV, which changed too:
# that was a third run of the whole generator. What changed and is reachable from what the
# generator read is changed for it at once (rich: Table.__rich_console__ ran 3 times, now 2).
import os
import reprlib

ENV = {"LINES": "24"}


class Console:
    environ = ENV

    def __init__(self):
        self.height = 10


def lines(con):
    for i in range(3):
        if con.height is None:
            yield con.environ["LINES"]
        else:
            yield con.height


hook = getattr(reprlib, os.environ.get("Z645_HOOK", "repr"))
c = Console()
got = []
for x in lines(c):
    got.append(x)
    hook(c)
    hook(ENV)
first = got[0]
# (taken before any unknown code ran)
#@value first: exactly={10}
last = x
# (con.height may be None after unknown code, and ENV["LINES"] anything)
#@value last: unknown
# (two runs of the generator, not three)
#@tabulations <= 5
#@work <= 65
