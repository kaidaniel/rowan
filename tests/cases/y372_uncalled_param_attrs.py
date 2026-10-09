"""An uncalled function reading attributes of its parameter: requirements, and a dict keyed by
data is open.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def render(user):
    name = user.name                         #@ input name
    tags = {t: True for t in user.tags}      #@ input tags
    _ = 0                                    #@row tags: present={} definite={} open
    return {"name": name, "tags": tags}
#@schema param render.user: always={name,tags}
