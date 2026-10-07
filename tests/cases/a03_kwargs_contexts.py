import json


def point(**kw):
    return {"x": 0, "y": 0, **kw}


point(z=1)["z"]                      #@ ok z
point()["z"]                         #@ missing z
point(**json.load(fp))["z"]          #@ input z
point(z=1)["x"]                      #@ ok x
#@schema json.load@L10: always={z}
