import requests

REQUIRED = {"id", "email"}


def fetch(url):
    resp = requests.get(url).json()
    for item in resp["data"]["items"]:
        item["id"]                                  #@ input id
        if not all(k in item for k in REQUIRED):
            continue
        item["email"]                               #@ ok email
    return resp.get("next")                         #@ none next


def validate(payload):
    if not REQUIRED <= payload.keys():
        raise ValueError("bad")
    return payload["id"], payload["email"], payload["name"]   #@ ok id ; ok email ; input name


fetch("http://x")
#@schema requests.get@L7['data']: always={items}
#@schema requests.get@L7['data']['items']['*']: always={id}
#@schema param validate.payload: always={name}
