# validation_alias (incl. AliasChoices) names the input keys, serialization_alias the dumped keys (by_alias=True); attributes keep the field names
from pydantic import AliasChoices, BaseModel, Field, ValidationError


class Event(BaseModel):
    event_type: str = Field(validation_alias="type")
    ts: int = Field(validation_alias=AliasChoices("ts", "timestamp"))
    user_id: int = Field(serialization_alias="userId")
    note: str = Field("", alias="n", serialization_alias="noteText")


def ingest(raw):
    e = Event.model_validate(raw)                #@ possibly type
    #@row e: present={event_type,ts,user_id,note} definite={event_type,ts,user_id,note} absent={type,timestamp,n} closed
    plain = e.model_dump()
    #@row plain: present={event_type,ts,user_id,note} definite={event_type,ts,user_id,note} closed
    wire = e.model_dump(by_alias=True)
    #@row wire: present={event_type,ts,userId,noteText} definite={event_type,ts,userId,noteText} absent={type,user_id,note,n} closed
    return wire["userId"]                        #@ ok userId


ev = Event(type="click", timestamp=7, user_id=1)
t = ev.event_type                                #@ ok event_type
#@value t: exactly={"click"}
ts = ev.ts
#@value ts: exactly={7}

if __name__ == "__main__":
    ingest({"type": "click", "ts": 1, "user_id": 1})
    ingest({"type": "view", "timestamp": 2, "user_id": 2, "n": "hi"})
    try:
        ingest({"event_type": "x", "ts": 3, "user_id": 3})
    except ValidationError:
        pass

#@schema param ingest.raw: always={type,user_id} sometimes={ts,timestamp} only={type,user_id,ts,timestamp}
