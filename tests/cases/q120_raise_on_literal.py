"""Hypothesis: a callee that raises for one literal input and returns for another makes
the caller's `except` branch alive for the first call and dead for the second.

Expectations were written from the program text before running the engine."""


def parse(text):
    if text == "":
        raise ValueError("empty")
    return {"text": text, "n": len(text)}


def safe(text):
    try:
        out = parse(text)
    except ValueError:
        out = {"error": True}
    return out


good = safe("abc")                        #@row good: present={text,n} definite={text,n} closed
bad = safe("")                            #@row bad: present={error} definite={error} closed
good["error"]                             #@ missing error
bad["text"]                               #@ missing text
