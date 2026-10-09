# a parse -> enrich -> store pipeline across functions: the dump dict grows keys, a strict model (extra="forbid") checks the stored shape
from pydantic import BaseModel, ConfigDict, ValidationError


class Incoming(BaseModel):
    user: str
    amount: float
    currency: str = "EUR"


class Stored(BaseModel):
    model_config = ConfigDict(extra="forbid")
    user: str
    amount: float
    currency: str
    amount_eur: float
    flagged: bool = False


RATES = {"EUR": 1.0, "USD": 0.9}


def parse(raw):
    return Incoming.model_validate(raw)          #@ possibly amount


def enrich(inc):
    d = inc.model_dump()
    #@row d: present={user,amount,currency} definite={user,amount,currency} closed
    d["amount_eur"] = d["amount"] * RATES.get(d["currency"], 1.0)    #@ ok amount ; ok currency
    if d["amount_eur"] > 1000:
        d["flagged"] = True
    #@row d: present={user,amount,currency,amount_eur,flagged} definite={user,amount,currency,amount_eur} closed
    return d


def store(d):
    s = Stored(**d)
    #@row s: present={user,amount,currency,amount_eur,flagged} definite={user,amount,currency,amount_eur,flagged} closed
    return s.model_dump()


def handle(raw):
    rec = store(enrich(parse(raw)))
    #@row rec: present={user,amount,currency,amount_eur,flagged} definite={user,amount,currency,amount_eur,flagged} closed
    return rec


big = handle({"user": "u", "amount": 2000})
fl = big["flagged"]
#@value fl: exactly={True}
small = handle({"user": "v", "amount": 10, "currency": "USD"})
fs = small["flagged"]
#@value fs: exactly={False}
ae = small["amount_eur"]
#@value ae: exactly={9.0}

if __name__ == "__main__":
    handle({"user": "w", "amount": 1, "note": "x"})
    try:
        handle({"user": "z"})
    except ValidationError:
        pass

#@schema param handle.raw: always={user,amount} only={user,amount}
