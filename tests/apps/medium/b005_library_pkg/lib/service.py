"""Service layer: validation under try/except with early returns of error records (keys ok,
error and what went wrong), loans through the repository and the rules, a pandas report of
loans per genre."""
import pandas as pd
from pydantic import ValidationError

from lib.models import BookIn, LoanIn
from lib.rules import MAX_LOANS, due_date, view


def add_books(repo, items):
    added, errors = [], {}
    for raw in items:
        try:
            book = BookIn.model_validate(raw)
        except ValidationError as e:
            errors[str(raw.get("isbn", "?"))] = sorted({err["loc"][0] for err in e.errors()})
            continue
        if book.isbn in repo.books:
            known = repo.books[book.isbn]
            known["copies"] += book.copies       #@ ok copies
            known["available"] += book.copies
            continue
        added.append(repo.add_book(book.model_dump())["isbn"])
    return {"added": added, "errors": errors}


def checkout(repo, payload, today):
    try:
        req = LoanIn.model_validate(payload)
    except ValidationError as e:
        return {"ok": False, "error": "invalid", "fields": [err["loc"][0] for err in e.errors()]}
    try:
        member = repo.member(req.member)
        book = repo.book(req.isbn)
    except LookupError as e:
        return {"ok": False, "error": "unknown", "what": str(e)}
    active = [ln for ln in repo.loans.values() if ln.member == req.member and ln.returned is None]
    if len(active) >= MAX_LOANS[member["tier"]]:  #@ ok tier
        return {"ok": False, "error": "limit"}
    if book["available"] == 0:                   #@ ok available
        return {"ok": False, "error": "unavailable"}
    book["available"] -= 1
    loan = repo.new_loan(req.member, req.isbn, today, due_date(today, req.days, member["tier"]))
    if req.note:
        loan.flags.append("note")
    res = {"ok": True, "loan": view(loan)}
    #@row res: present={ok,loan} definite={ok,loan} closed
    return res


def give_back(repo, loan_id, today):
    loan = repo.loans.get(loan_id)
    if loan is None or loan.returned is not None:
        return None
    member = repo.member(loan.member)
    out = view(loan, today, member["tier"])
    loan.returned = today
    repo.book(loan.isbn)["available"] += 1
    return out


def loans_frame(repo):
    rows = []
    for loan in repo.loans.values():
        book = repo.book(loan.isbn)
        rows.append({"loan": loan.id, "member": loan.member, "genre": book["genre"], "open": loan.returned is None})
    if not rows:
        return pd.DataFrame(columns=["genre", "loans", "open"])
    df = pd.DataFrame(rows)
    #@row df: present={loan,member,genre,open} definite={loan,member,genre,open} closed
    per = df.groupby("genre", as_index=False).agg(loans=("loan", "count"), open=("open", "sum"))
    #@row per: present={genre,loans,open} definite={genre,loans,open} closed
    return per
