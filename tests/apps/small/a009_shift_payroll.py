# Payroll: a pydantic model (alias, Literal with default, ge constraint, field validator) validated from literal records and from JSON in os.environ under try/except, a helper that adds pay keys conditionally, the rows into a pandas frame grouped by a field
"""Hypothesis: invalid records are collected per index with the failing fields (by alias:
"name"); the pay row of a shift is the dump minus `overtime` plus `pay`, and `ot` only when the
shift had overtime: decided per literal record (exactly 220.0 for 8 h + 2 h overtime at the
default rate 20.0), present-only for records from input. The frame's columns are the union of
the rows' keys (`ot` definite for the literal shifts, present-only from input); the groupby
result has exactly the group key and the two named aggregates. The JSON input must give `name`
and `hours` in every record (missing fields are caught but still required)."""
import json
import os
from typing import Literal, Optional

import pandas as pd
from pydantic import BaseModel, Field, ValidationError, field_validator


class Shift(BaseModel):
    worker: str = Field(alias="name")
    team: Literal["ops", "dev", "qa"] = "ops"
    hours: float = Field(ge=0)
    rate: float = 20.0
    overtime: Optional[float] = None

    @field_validator("worker")
    @classmethod
    def clean(cls, v):
        return v.strip().title()


def parse_shifts(records):
    ok, rejected = [], {}
    for i, raw in enumerate(records):
        try:
            ok.append(Shift.model_validate(raw))
        except ValidationError as e:
            rejected[i] = [err["loc"][0] for err in e.errors()]
    return ok, rejected


def pay_row(s):
    row = s.model_dump(exclude={"overtime"})
    #@row row: present={worker,team,hours,rate} definite={worker,team,hours,rate} absent={overtime,name} closed
    row["pay"] = round(s.hours * s.rate, 2)
    if s.overtime:
        row["pay"] += round(s.overtime * s.rate * 1.5, 2)
        row["ot"] = True
    return row


def frame(shifts):
    rows = [pay_row(s) for s in shifts]
    if not rows:
        return None
    return pd.DataFrame(rows)


def by_team(df):
    out = df.groupby("team", as_index=False).agg(total=("pay", "sum"), people=("worker", "nunique"))
    #@row out: present={team,total,people} definite={team,total,people} closed
    return out


def overtime_workers(df):
    return sorted(df[df["ot"] == True]["worker"])  #@ possibly ot


if __name__ == "__main__":
    good, rej = parse_shifts([
        {"name": " ann lee ", "team": "dev", "hours": 8, "overtime": 2},
        {"name": "bo", "hours": 6.5},
        {"name": "cy", "team": "hr", "hours": 4},
        {"team": "qa", "hours": -1},
    ])
    n = len(good)                                #@value n: exactly={2}
    bad = tuple(sorted(rej))                     #@value bad: exactly={(2,3)}
    why3 = tuple(rej[3])                         #@value why3: exactly={("name","hours")}
    w0 = good[0].worker                          #@value w0: exactly={"Ann Lee"}
    rate = good[1].rate                          #@value rate: exactly={20.0}
    r0 = pay_row(good[0])
    #@row r0: present={worker,team,hours,rate,pay,ot} definite={worker,team,hours,rate,pay,ot} closed
    p0 = r0["pay"]                               #@value p0: exactly={220.0}
    r1 = pay_row(good[1])
    #@row r1: present={worker,team,hours,rate,pay} definite={worker,team,hours,rate,pay} absent={ot} closed
    t1 = r1["team"]                              #@value t1: exactly={ops}
    df = frame(good)
    #@row df: present={worker,team,hours,rate,pay,ot} definite={worker,team,hours,rate,pay,ot} closed
    teams = by_team(df)
    for d in (df, frame(good[1:])):
        try:
            overtime_workers(d)
        except KeyError:
            pass

    raw = json.loads(os.environ.get("SHIFTS_JSON", '[{"name": "dee", "team": "qa", "hours": 7, "overtime": 1}, {"name": "eve", "hours": 3}]'))
    #@schema json.loads@L94['*']: always={name,hours} only={name,hours}
    good2, rej2 = parse_shifts(raw)
    df2 = frame(good2)
    if df2 is not None:
        _ = 0  #@row df2: present={worker,team,hours,rate,pay,ot} definite={worker,team,hours,rate,pay} closed
        teams2 = by_team(df2)
