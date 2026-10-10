"""Hypothesis: a model_validator(mode="before") that renames a legacy key on a copy decides the
fields (the renamed key is not an extra, the caller's dict keeps its old key) and the renamed
field counts as set; extra="allow" puts unknown keys after the fields in a dump; an after
validator that raises for some field values gives a caught ValidationError of type value_error, a missing field one of type missing. Input records may take every branch: the result row is
open (extras) with nothing definite; start is required (a caught missing still counts), kind
unless type is given (corrected: was "nothing is required", against the pydantic convention)."""
import json
import os

from pydantic import BaseModel, ConfigDict, ValidationError, model_validator


class Event(BaseModel):
    model_config = ConfigDict(extra="allow")
    kind: str
    start: int
    end: int | None = None

    @model_validator(mode="before")
    @classmethod
    def legacy(cls, data):
        if isinstance(data, dict) and "type" in data:
            data = dict(data)
            data["kind"] = data.pop("type")
        return data

    @model_validator(mode="after")
    def check_order(self):
        if self.end is not None and self.end < self.start:
            raise ValueError("end before start")
        return self


def load(raw):
    try:
        ev = Event.model_validate(raw)
    except ValidationError as e:
        return {"error": e.errors()[0]["type"]}
    return ev.model_dump()


if __name__ == "__main__":
    raw1 = {"type": "a", "start": 1}
    r1 = load(raw1)
    #@row r1: present={kind,start,end} definite={kind,start,end} absent={type,error} closed
    k1 = r1["kind"]                              #@value k1: exactly={"a"}
    e1 = r1["end"]                               #@value e1: exactly={None}
    _ = 0                                        #@row raw1: present={type,start} definite={type,start} absent={kind} closed
    fs = tuple(sorted(Event.model_validate(raw1).model_fields_set))   #@value fs: exactly={("kind","start")}
    r2 = load({"kind": "b", "start": 1, "end": 5, "room": "r1"})
    #@row r2: present={kind,start,end,room} definite={kind,start,end,room} closed
    order = tuple(r2)                            #@value order: exactly={("kind","start","end","room")}
    room = r2["room"]                            #@value room: exactly={"r1"}
    r3 = load({"kind": "c", "start": 5, "end": 2})
    #@row r3: present={error} definite={error} closed
    err3 = r3["error"]                           #@value err3: exactly={"value_error"}
    err4 = load({"start": 1})["error"]           #@value err4: exactly={"missing"}
    r5 = load(json.loads(os.environ.get("EVENT", '{"type": "z", "start": 3, "tag": 1}')))
    #@schema json.loads@L58: always={start} sometimes={kind} only={kind,start,type}
    _ = 0                                        #@row r5: possible={kind,start,end,error} definite={} open
#@todo row r5: present={kind,start,end,error} definite={} open  (type is popped by the before validator or absent: never an extra)
