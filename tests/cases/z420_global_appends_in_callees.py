# appends to a module list from a callee of a summarised function: positions are those of each call site
OUT = []
LOG = []


def send(to, ctx):
    msg = {"to": to}
    if "plan" in ctx:
        msg["plan"] = ctx["plan"]
    OUT.append(msg)
    LOG.append(to)


def handle(body):
    email = body["email"]
    send(email, body)
    return LOG[-1]


for body in [{"email": "a"}, {"email": "b", "plan": "pro"}]:
    last = handle(body)                              #@value last: exactly={a,b}
first = OUT[0]
#@row first: present={to} definite={to} closed
second = OUT[1]
#@row second: present={to,plan} definite={to,plan} closed
n = len(OUT)                                         #@value n: exactly={2}
who = LOG[1]                                         #@value who: exactly={b}
