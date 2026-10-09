"""Hypothesis: f-strings with literal parts and a parameter build keys that are decided per
call site, and nested f-string pieces concatenate correctly.

Expectations were written from the program text before running the engine."""


def key_for(kind, n):
    return f"{kind}:{n:03d}"


def label(prefix, kind):
    return f"{prefix}-{kind}"


table = {key_for("lot", 7): {"n": 7}, key_for("box", 12): {"n": 12}}  #@row table: present={lot:007,box:012} definite={lot:007,box:012} closed
labels = {label("wh", "a"): 1, label("wh", "b"): 2}  #@row labels: present={wh-a,wh-b} definite={wh-a,wh-b} closed
table["lot:7"]                            #@ missing lot:7
table["lot:007"]                          #@ ok lot:007
