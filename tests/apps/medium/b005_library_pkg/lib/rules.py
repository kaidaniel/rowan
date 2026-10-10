"""Rules: gold members keep books a week longer and pay no late fee; the view of a loan drops
`returned` while it is open and `flags` while empty, and adds `fee` / `overdue` only when a
fee is due on the given day."""
from dataclasses import asdict
from datetime import timedelta

FEE_PER_DAY = {"gold": 0.0, "basic": 0.5}
MAX_LOANS = {"gold": 5, "basic": 1}


def due_date(start, days, tier):
    if tier == "gold":
        days += 7
    return start + timedelta(days=days)


def fee(loan, today, tier):
    late = (today - loan.due).days
    if late <= 0:
        return 0.0
    return round(late * FEE_PER_DAY[tier], 2)


def view(loan, today=None, tier="basic"):
    out = asdict(loan)
    #@row out: present={id,member,isbn,start,due,returned,flags} definite={id,member,isbn,start,due,returned,flags} closed
    out["start"] = loan.start.isoformat()
    out["due"] = loan.due.isoformat()
    if loan.returned is None:
        del out["returned"]
    else:
        out["returned"] = loan.returned.isoformat()
    if not loan.flags:
        out.pop("flags")
    if today is not None and loan.returned is None:
        f = fee(loan, today, tier)
        if f > 0:
            out["fee"] = f
            out["overdue"] = True
    _ = 0  #@row out: present={id,member,isbn,start,due,returned,flags,fee,overdue} definite={id,member,isbn,start,due} closed
    return out
