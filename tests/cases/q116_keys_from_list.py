"""Hypothesis: a dict built in a loop over a literal list of keys, with values from a
function, has exactly those keys, and reading one back gives that function's record.

Expectations were written from the program text before running the engine."""

FIELDS = ["name", "age", "city"]


def describe(field):
    return {"field": field, "required": field != "city"}


schema = {}
for f in FIELDS:
    schema[f] = describe(f)
_ = 0                                     #@row schema: present={name,age,city} definite={name,age,city} closed
city = schema["city"]                     #@row city: present={field,required} definite={field,required} closed
schema["zip"]                             #@ missing zip
