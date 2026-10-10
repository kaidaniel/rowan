"""A hook registry: rules register callbacks per event with a decorator; fire merges the
notes the callbacks return, in registration order."""
from collections import defaultdict

HOOKS = defaultdict(list)


def on(event):
    def deco(fn):
        HOOKS[event].append(fn)
        return fn
    return deco


def fire(event, task, ctx):
    notes = {}
    for fn in HOOKS[event]:
        notes.update(fn(task, ctx))
    _ = 0  #@row notes: present={escalated,due_hint,due_in_days,closed_by,late} definite={} closed
    return notes
