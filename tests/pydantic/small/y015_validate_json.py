# model_validate_json on text and model_validate on json.loads: the same instance; the json.loads input must hold the required keys
import json

from pydantic import BaseModel, ValidationError


class Reading(BaseModel):
    sensor: str
    value: float
    unit: str = "C"


def parse_text(text):
    r = Reading.model_validate_json(text)        #@ possibly value
    #@row r: present={sensor,value,unit} definite={sensor,value,unit} closed
    return r


def parse_loaded(text):
    raw = json.loads(text)
    # Corrected on re-reading: every caller passes one of the literal TEXTS, so in every run
    # `raw` is one of their (closed) parses; the schema below is the function's own contract.
    #@row raw: present={sensor,value,unit,x} definite={sensor,value} closed
    r = Reading.model_validate(raw)
    #@row r: present={sensor,value,unit} definite={sensor,value,unit} closed
    return r.unit


fixed = Reading.model_validate_json('{"sensor": "s1", "value": 2.5}')
#@row fixed: present={sensor,value,unit} definite={sensor,value,unit} closed
u = fixed.unit
#@value u: exactly={"C"}

TEXTS = ['{"sensor": "a", "value": 1}', '{"sensor": "b", "value": 2, "unit": "F", "x": 1}']

if __name__ == "__main__":
    for t in TEXTS:
        parse_text(t)
        parse_loaded(t)
    try:
        parse_text('{"sensor": "c"}')
    except ValidationError:
        pass

#@schema json.loads@L20: always={sensor,value} only={sensor,value}
