def handler(event, context):
    body = event["body"]
    user_id = body["user"]["id"]
    if event.get("debug"):
        return {"id": user_id, "debug": True}
    return {"id": user_id}


def helper(d):
    return d["q"]
#@schema param handler.event: always={body}
#@schema param handler.event['body']: always={user}
#@schema param handler.event['body']['user']: always={id}
#@schema param helper.d: always={q}
