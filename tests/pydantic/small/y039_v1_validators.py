# v1 validators in v2: @validator (pre=True too), @root_validator(pre=True) gets the raw input dict, @root_validator(skip_on_failure=True) the field values
import warnings

from pydantic import BaseModel, ValidationError, root_validator, validator

warnings.simplefilter("ignore", DeprecationWarning)


class Transfer(BaseModel):
    src: str
    dst: str
    amount: float
    currency: str = "EUR"

    @root_validator(pre=True)
    def legacy_names(cls, values):
        if "from" in values:
            values = dict(values)
            values["src"] = values.pop("from")
        # Corrected on re-reading: this directive applies to the store above (inside the `if`);
        # every input there has dst and currency too.
        #@row values: present={src,dst,amount,currency} definite={src,dst,amount,currency} absent={from} closed
        return values

    @validator("currency", pre=True)
    def upper(cls, v):
        return v.upper()

    @validator("amount")
    def positive(cls, v):
        if v <= 0:
            raise ValueError("amount must be positive")
        return v

    @root_validator(skip_on_failure=True)
    def distinct(cls, values):
        # Corrected on re-reading: a directive line below the `def` applied to the def statement;
        # moved below the first body line.
        if values["src"] == values["dst"]:       #@ ok src ; ok dst
        #@row values: present={src,dst,amount,currency} definite={src,dst,amount,currency} closed
            raise ValueError("same account")
        return values


def transfer(d):
    t = Transfer(**d)                            #@ possibly dst
    #@row t: present={src,dst,amount,currency} definite={src,dst,amount,currency} absent={from} closed
    return t


t = Transfer(**{"from": "a", "dst": "b", "amount": 1, "currency": "usd"})
cur = t.currency
#@value cur: exactly={"USD"}
src = t.src
#@value src: exactly={"a"}

if __name__ == "__main__":
    transfer({"src": "x", "dst": "y", "amount": 2})
    transfer({"from": "x", "dst": "z", "amount": 3, "currency": "gbp"})
    for bad in ({"src": "x", "amount": 1}, {"src": "x", "dst": "x", "amount": 1}, {"src": "x", "dst": "y", "amount": -1}):
        try:
            transfer(bad)
        except ValidationError:
            pass

# Corrected on re-reading: `from` is read only behind `"from" in values` (dropping it alone
# never fails for it), so per key only `src` is sometimes required.
#@schema param transfer.d: always={dst,amount} sometimes={src} only={dst,amount,src,from}
