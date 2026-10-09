# string.Template: substitute reads every placeholder (KeyError when absent), safe_substitute does not
from string import Template

T = Template("Hello $name: you owe $$$amount")


def greet(rec):
    return T.substitute(rec)                     #@ ok name ; ok amount ; missing amount
#@schema param greet.rec: always={name,amount} only={name,amount}


def soft(rec):
    return T.safe_substitute(rec)                #@ none amount


def with_kw(rec):
    return T.substitute(rec, amount=0)           #@ ok name


if __name__ == "__main__":
    g = greet({"name": "a", "amount": 3})        #@value g: exactly={"Hello a: you owe $3"}
    try:
        greet({"name": "b"})
    except KeyError:
        pass
    s = soft({"name": "b"})                      #@value s: exactly={"Hello b: you owe $$amount"}
    k = with_kw({"name": "c", "amount": 9})      #@value k: exactly={"Hello c: you owe $0"}
    with_kw({"name": "d"})
    ids = tuple(T.get_identifiers())             #@value ids: exactly={("name","amount")}
