# M(**d) with dicts built by callers: extra keys ignored, a required key absent on one path, keyword defaults merged before the call
from pydantic import BaseModel, ValidationError


class Ticket(BaseModel):
    title: str
    priority: int = 3
    assignee: str | None = None


def open_ticket(d, **defaults):
    t = Ticket(**{**defaults, **d})              #@ possibly title
    #@row t: present={title,priority,assignee} definite={title,priority,assignee} closed
    return t


def from_email(msg):
    d = {"title": msg["subject"]}                #@ possibly subject
    if msg.get("urgent"):
        d["priority"] = 1
    #@row d: present={title,priority} definite={title} closed
    return open_ticket(d, assignee="triage")


def from_form(form):
    return open_ticket(form)


t = from_email({"subject": "s", "urgent": True})
pr = t.priority
#@value pr: exactly={1}
asg = t.assignee
#@value asg: exactly={"triage"}

if __name__ == "__main__":
    from_email({"subject": "x"})
    try:
        from_email({"body": "no subject"})
    except KeyError:
        pass
    from_form({"title": "f", "priority": 2, "extra": 1})
    try:
        from_form({"priority": 2})
    except ValidationError:
        pass

#@schema param from_email.msg: always={subject} only={subject}
#@schema param from_form.form: always={title} only={title}
