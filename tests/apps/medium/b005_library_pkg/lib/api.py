"""HTTP layer: FastAPI routes over one repository given by a dependency; the service's error
records become HTTPExceptions with the record as detail and a status code per error."""
from datetime import date

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel

from lib.repo import Repo
from lib.service import add_books, checkout, give_back, loans_frame

TODAY = date(2024, 5, 1)
REPO = Repo()
STATUS = {"invalid": 422, "unknown": 404, "limit": 409, "unavailable": 409}

app = FastAPI()


def get_repo():
    return REPO


class BooksIn(BaseModel):
    items: list[dict]


@app.post("/books")
def post_books(body: BooksIn, repo: Repo = Depends(get_repo)):
    res = add_books(repo, body.items)
    #@row res: present={added,errors} definite={added,errors} closed
    return res


@app.get("/books/{isbn}")
def get_book(isbn: str, repo: Repo = Depends(get_repo)):
    try:
        return repo.book(isbn)
    except LookupError:
        raise HTTPException(status_code=404, detail="no such book")


@app.post("/loans", status_code=201)
def post_loan(payload: dict, repo: Repo = Depends(get_repo)):
    res = checkout(repo, payload, TODAY)
    if not res["ok"]:                            #@ ok ok
        raise HTTPException(status_code=STATUS[res["error"]], detail=res)  #@ ok error
    return res["loan"]


@app.post("/loans/{loan_id}/return")
def post_return(loan_id: int, today: str = "2024-05-01", repo: Repo = Depends(get_repo)):
    out = give_back(repo, loan_id, date.fromisoformat(today))
    if out is None:
        raise HTTPException(status_code=404, detail="no open loan")
    return out


@app.get("/report")
def report(repo: Repo = Depends(get_repo)):
    per = loans_frame(repo)
    return {"genres": per.to_dict(orient="records")}
