# Lending library over a package: FastAPI routes (lib.api) -> service layer (lib.service: validation under try/except, early error records, a pandas report) -> rules (lib.rules: due dates, fees, a loan view that drops / adds keys) -> repository dicts (lib.repo) and models (lib.models: pydantic + dataclass); TestClient requests from literals, a loan request and books from JSON in os.environ
"""Hypothesis: every response follows from the literal requests: the books batch adds exactly
the two valid new isbns ("978-1" normalised to "9781", a duplicate adding copies), errors keyed
by the rejected isbns with the failing fields; loans get due dates by tier (basic 14 days, gold
+7), a limit / unavailable / invalid / unknown request is a 409 / 409 / 422 / 404 whose detail is
the service's error record (its keys exact). A returned loan's view has `fee` and `overdue` only
when late (2.5 for 5 days at 0.5), `flags` only when a note was given, never `returned` (the view
is taken before the return is recorded). The report frame has exactly genre, loans, open. A loan
request from input must give `member` and `isbn`; books from input `isbn` and `title`."""
import json
import os
from datetime import date

from fastapi.testclient import TestClient

from lib.api import REPO, app
from lib.rules import view
from lib.service import add_books, checkout

if __name__ == "__main__":
    client = TestClient(app)
    r = client.post("/books", json={"items": [
        {"isbn": "978-1", "title": "Dune", "copies": 1},
        {"isbn": "9782", "title": "Cosmos", "genre": "science", "copies": 2},
        {"isbn": "12", "title": "Short"},
        {"isbn": "abcd", "title": "Letters", "genre": "poetry"},
        {"isbn": "9781", "title": "Dune again", "copies": 2},
    ]}).json()
    #@row r: present={added,errors} definite={added,errors} closed
    added = tuple(r["added"])                    #@value added: exactly={("9781","9782")}
    errs = r["errors"]
    #@row errs: present={"12",abcd} definite={"12",abcd} closed
    why = tuple(errs["abcd"])                    #@value why: exactly={("genre","isbn")}
    dune = client.get("/books/9781").json()
    #@row dune: present={isbn,title,genre,copies,available} definite={isbn,title,genre,copies,available} closed
    avail = dune["available"]                    #@value avail: exactly={3}
    genre = dune["genre"]                        #@value genre: exactly={fiction}
    nf = client.get("/books/0000").status_code   #@value nf: exactly={404}

    l1 = client.post("/loans", json={"member": "m2", "isbn": "9782", "note": "gift"})
    c1 = l1.status_code                          #@value c1: exactly={201}
    loan1 = l1.json()
    #@row loan1: present={id,member,isbn,start,due,flags} definite={id,member,isbn,start,due,flags} absent={returned,fee,overdue} closed
    due1 = loan1["due"]                          #@value due1: exactly={"2024-05-15"}
    lim = client.post("/loans", json={"member": "m2", "isbn": "9781"})
    lc = lim.status_code                         #@value lc: exactly={409}
    ld = lim.json()["detail"]
    #@row ld: present={ok,error} definite={ok,error} closed
    le = ld["error"]                             #@value le: exactly={limit}
    l2 = client.post("/loans", json={"member": "m1", "isbn": "9782", "days": 10}).json()
    #@row l2: present={id,member,isbn,start,due} definite={id,member,isbn,start,due} absent={flags,returned} closed
    due2 = l2["due"]                             #@value due2: exactly={"2024-05-18"}
    un = client.post("/loans", json={"member": "m1", "isbn": "9782"}).json()["detail"]["error"]   #@value un: exactly={unavailable}
    bad = client.post("/loans", json={"member": "m1", "isbn": "9781", "days": 40})
    bc = bad.status_code                         #@value bc: exactly={422}
    bf = tuple(bad.json()["detail"]["fields"])   #@value bf: exactly={("days",)}
    who = client.post("/loans", json={"member": "m9", "isbn": "9781"}).json()["detail"]
    #@row who: present={ok,error,what} definite={ok,error,what} closed
    what = who["what"]                           #@value what: exactly={"member m9"}

    back = client.post("/loans/1/return", params={"today": "2024-05-20"}).json()
    #@row back: present={id,member,isbn,start,due,flags,fee,overdue} definite={id,member,isbn,start,due,flags,fee,overdue} absent={returned} closed
    fee1 = back["fee"]                           #@value fee1: exactly={2.5}
    again = client.post("/loans/1/return").status_code      #@value again: exactly={404}
    b2 = client.post("/loans/2/return").json()
    #@row b2: present={id,member,isbn,start,due} definite={id,member,isbn,start,due} absent={fee,overdue,returned,flags} closed
    rep = client.get("/report").json()
    #@row rep: present={genres} definite={genres} closed
    g0 = rep["genres"][0]
    #@row g0: present={genre,loans,open} definite={genre,loans,open} closed
    v1 = view(REPO.loans[1])
    #@row v1: present={id,member,isbn,start,due,returned,flags} definite={id,member,isbn,start,due,returned,flags} absent={fee,overdue} closed

    payload = json.loads(os.environ.get("LOAN_JSON", '{"member": "m1", "isbn": "9781", "days": 7}'))
    #@schema json.loads@L74: always={member,isbn} only={member,isbn}
    res = checkout(REPO, payload, date(2024, 6, 1))
    #@row res: present={ok,error,fields,what,loan} definite={ok} closed
    books = json.loads(os.environ.get("BOOKS_JSON", '[{"isbn": "5555", "title": "SPQR", "genre": "history"}, {"isbn": "55", "title": "x"}]'))
    #@schema json.loads@L78['*']: always={isbn,title} only={isbn,title}
    out = add_books(REPO, books)
    #@row out: present={added,errors} definite={added,errors} closed
    client.get("/report")
