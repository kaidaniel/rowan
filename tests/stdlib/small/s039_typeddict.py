# TypedDict (total=False / Required / NotRequired): plain dicts at run time; required key sets
from typing import NotRequired, Required, TypedDict


class Movie(TypedDict):
    title: str
    year: int


class Opts(TypedDict, total=False):
    sort: str
    limit: int


class Req(TypedDict, total=False):
    id: Required[int]
    note: str


class Rec(TypedDict):
    id: int
    extra: NotRequired[str]


def show(m: Movie) -> str:
    return f"{m['title']} ({m['year']})"         #@ ok title ; ok year
#@schema param show.m: always={title,year} only={title,year}


def query(opts: Opts):
    sort = opts.get("sort", "title")             #@ none sort
    limit = opts["limit"] if "limit" in opts else 10
    return {"sort": sort, "limit": limit}


if __name__ == "__main__":
    m = Movie(title="x", year=1)                 #@row m: present={title,year} definite={title,year} closed
    s = show(m)                                  #@value s: exactly={"x (1)"}
    show({"title": "y", "year": 2, "rating": 5})
    q1 = query({})                               #@row q1: present={sort,limit} definite={sort,limit} closed
    q2 = query({"limit": 3})
    lim = q2["limit"]                            #@value lim: exactly={3}
    rk = tuple(sorted(Req.__required_keys__))    #@value rk: exactly={("id",)}
    ok_ = tuple(sorted(Req.__optional_keys__))   #@value ok_: exactly={("note",)}
    rr = tuple(sorted(Rec.__required_keys__))    #@value rr: exactly={("id",)}
    partial = Movie(title="z")                   #@row partial: present={title} definite={title} closed
