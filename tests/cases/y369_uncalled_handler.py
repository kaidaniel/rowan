"""An uncalled handler: keys it builds are exact, reads of its parameter are requirements.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def handler(event):
    out = {"ok": True}
    if event.get("debug"):
        out["debug"] = event["debug"]        #@ ok debug
    _ = 0                                    #@row out: present={ok,debug} definite={ok} closed
    body = event["body"]                     #@ input body
    return out
#@schema param handler.event: always={body}
