"""NamedTuple fields with a default, a TypedDict with a non-total subclass, functions
annotated with them (both called and only reachable from the entry-point pass).

Expectations were written from the program text before running the engine."""
from typing import NamedTuple, TypedDict


class Point(NamedTuple):
    x: int
    y: int
    label: str = ""


class Movie(TypedDict):
    title: str
    year: int


class MovieExtra(Movie, total=False):
    rating: float


def make_movie(title, year, rating=None):
    m: MovieExtra = {"title": title, "year": year}
    if rating is not None:
        m["rating"] = rating
    return m


def show(m: Movie):
    return {"line": m["title"], "y": m["year"]}


def stars(m: MovieExtra):
    return m.get("rating", 0)


def headline(m: Movie):
    return m["title"].upper()            #@ ok title


def bogus(m: Movie):
    return m["director"]                 #@ missing director


p = Point(1, 2)                           #@row p: present={x,y,label} definite={x,y,label} closed
m1 = make_movie("a", 1999)                #@row m1: present={title,year} definite={title,year} closed
m2 = make_movie("b", 2000, 4.5)           #@row m2: present={title,year,rating} definite={title,year,rating} closed
s = show(m1)                              #@row s: present={line,y} definite={line,y} closed
r = stars(m2)
m1["rating"]                              #@ missing rating
p.label                                   #@ ok label
p.z                                       #@ missing z
