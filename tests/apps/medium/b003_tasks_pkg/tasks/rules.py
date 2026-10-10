"""Rules, registered when this module is imported (tasks.api imports it)."""
from tasks.hooks import on
from tasks.models import Priority


@on("created")
def escalate(task, ctx):
    if task.priority == Priority.HIGH or "urgent" in task.labels:
        return {"escalated": True}
    return {}


@on("created")
def due_hint(task, ctx):
    if task.due is None:
        return {"due_hint": "none"}
    return {"due_in_days": (task.due - ctx["today"]).days}   #@ ok today


@on("completed")
def closer(task, ctx):
    out = {"closed_by": ctx["user"]}                         #@ ok user
    if task.due is not None and ctx["today"] > task.due:
        out["late"] = True
    return out
