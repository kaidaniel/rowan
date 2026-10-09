# typing.NamedTuple with defaults, its helpers, and get_type_hints of classes and functions
from typing import NamedTuple, TypedDict, get_type_hints


class Employee(NamedTuple):
    name: str
    dept: str = "eng"
    salary: int = 0


class Movie(TypedDict):
    title: str
    year: int


def raise_pay(e, amount):
    e2 = e._replace(salary=e.salary + amount)    #@row e2: present={name,dept,salary} definite={name,dept,salary} closed
    return e2


def route(a: int, b: str = "") -> bool:
    return bool(a)


if __name__ == "__main__":
    e = Employee("a")                            #@row e: present={name,dept,salary} definite={name,dept,salary} closed
    r = raise_pay(e, 10)
    s = r.salary                                 #@value s: exactly={10}
    r2 = raise_pay(Employee("b", "ops", 5), 1)
    d = r2.dept                                  #@value d: exactly={ops}
    dflt = Employee._field_defaults              #@row dflt: present={dept,salary} definite={dept,salary} closed
    asd = e._asdict()                            #@row asd: present={name,dept,salary} definite={name,dept,salary} closed
    try:
        e.bonus                                  #@ missing bonus
    except AttributeError:
        pass
    h = get_type_hints(Employee)                 #@row h: present={name,dept,salary} definite={name,dept,salary} closed
    hf = get_type_hints(route)                   #@row hf: present={a,b,return} definite={a,b,return} closed
    hm = get_type_hints(Movie)                   #@row hm: present={title,year} definite={title,year} closed
