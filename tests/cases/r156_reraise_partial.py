"""Hypothesis: a bare `raise` in an inner handler propagates to the outer handler; keys
stored before the raise (in the callee and in the inner handler) survive, keys after it
do not.

Expectations were written from the program text before running the engine."""


def step(log, fail):
    log["started"] = True
    if fail:
        raise RuntimeError("x")
    log["finished"] = True


def run(fail):
    log = {}
    try:
        try:
            step(log, fail)
        except RuntimeError:
            log["inner"] = True
            raise
    except RuntimeError:
        log["outer"] = True
    return log


ok = run(False)                              #@row ok: present={started,finished} definite={started,finished} closed
bad = run(True)                              #@row bad: present={started,inner,outer} definite={started,inner,outer} closed
ok["outer"]                                  #@ missing outer
bad["finished"]                              #@ missing finished
