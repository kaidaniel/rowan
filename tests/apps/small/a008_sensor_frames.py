# Sensor readings: a pydantic model (alias, Literal, Optional default, before / after model validators, field validator) validated from literal records and from JSON in os.environ, dumped with exclude_none / exclude, into pandas frames (from records, assign, groupby agg, merge)
"""Hypothesis: a model validated in a loop, its ValidationErrors collected, and dumped with
exclude_none: the Optional field is a present-only key of each dump, decided per record for
literal records; the frame built from the dumps has the union of the dump keys as columns
(present-only `note` when the records come from input). The before-validator reads the raw
input, so a legacy key can stand in for a required field (a `sometimes` requirement)."""
import json
import os
from typing import Literal, Optional

import pandas as pd
from pydantic import BaseModel, Field, ValidationError, field_validator, model_validator

ROOMS = pd.DataFrame({"sensor": ["t1", "t2"], "room": ["lab", "hall"]})


class Reading(BaseModel):
    sensor: str = Field(alias="id")
    value: float
    unit: Literal["C", "F"] = "C"
    note: Optional[str] = None
    tags: list[str] = []

    @model_validator(mode="before")
    @classmethod
    def legacy(cls, data):
        if isinstance(data, dict) and "temp" in data:
            data = {k: v for k, v in data.items() if k != "temp"} | {"value": data["temp"]}
        return data

    @field_validator("sensor")
    @classmethod
    def norm(cls, v):
        return v.strip().lower()

    @model_validator(mode="after")
    def to_celsius(self):
        if self.unit == "F":
            self.value = round((self.value - 32) * 5 / 9, 2)
            self.unit = "C"
        return self


def load(records):
    out, bad = [], []
    for raw in records:
        try:
            out.append(Reading.model_validate(raw))
        except ValidationError as e:
            bad.append(e.errors()[0]["loc"][0])
    return out, bad


def dump(r):
    d = r.model_dump(exclude_none=True, exclude={"tags"})
    #@row d: present={sensor,value,unit,note} definite={sensor,value,unit} absent={tags,id} closed
    return d


def to_frame(readings):
    if not readings:
        return pd.DataFrame(columns=["sensor", "value", "unit"])
    df = pd.DataFrame([dump(r) for r in readings])
    df["n_tags"] = [len(r.tags) for r in readings]
    return df


def per_room(df):
    agg = df.groupby("sensor").agg(mean=("value", "mean"), n=("value", "size")).reset_index()
    #@row agg: present={sensor,mean,n} definite={sensor,mean,n} closed
    return agg.merge(ROOMS, on="sensor", how="left")


def noted(df):
    sensors = set(df["sensor"])                  #@ ok sensor
    return df[df["note"].notna()]                #@ possibly note


if __name__ == "__main__":
    good, bad = load([
        {"id": " T1 ", "value": 21.5},
        {"id": "t1", "value": 98.6, "unit": "F", "note": "probe"},
        {"id": "t2", "temp": 19, "tags": ["roof", "north"]},
        {"id": "t9", "value": 3, "unit": "K"},
        {"value": 4},
    ])
    ng = len(good)                               #@value ng: exactly={3}
    errs = tuple(bad)                            #@value errs: exactly={("unit","id")}
    r1 = good[1]
    #@row r1: present={sensor,value,unit,note,tags} definite={sensor,value,unit,note,tags} closed
    c = r1.value                                 #@value c: exactly={37.0}
    s0 = good[0].sensor                          #@value s0: exactly={t1}
    v2 = good[2].value                           #@value v2: exactly={19.0}
    d0 = dump(good[0])
    #@row d0: present={sensor,value,unit} definite={sensor,value,unit} absent={note,tags} closed
    d1 = dump(r1)
    #@row d1: present={sensor,value,unit,note} definite={sensor,value,unit,note} closed
    al = good[2].model_dump(by_alias=True, include={"sensor", "tags"})
    #@row al: present={id,tags} definite={id,tags} closed
    df = to_frame(good)
    #@row df: present={sensor,value,unit,note,n_tags} definite={sensor,value,unit,note,n_tags} closed
    rooms = per_room(df)
    #@row rooms: present={sensor,mean,n,room} definite={sensor,mean,n,room} closed

    raw = json.loads(os.environ.get("READINGS_JSON", '[{"id": "t2", "value": 18.0}, {"id": "t3", "temp": 70, "unit": "F"}]'))
    #@schema json.loads@L105['*']: always={id} sometimes={value} only={id,value}
    more, _ = load(raw)
    df2 = to_frame(more)
    #@row df2: present={sensor,value,unit,note,n_tags} definite={sensor,value,unit} closed
    rooms2 = per_room(df2)
    #@row rooms2: present={sensor,mean,n,room} definite={sensor,mean,n,room} closed
    m0 = more[0]
    #@row m0: present={sensor,value,unit,note,tags} definite={sensor,value,unit,note,tags} closed
    u0 = m0.unit                                 #@value u0: exactly={C}
    for frame in (df, df2):
        try:
            noted(frame)
        except KeyError:
            pass
