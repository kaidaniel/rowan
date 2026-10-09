"""Hypothesis: a function never called is analysed with open parameters: its reads are a
schema on the parameter, per path, sometimes when only one branch reads.

Expectations were written from the program text before running the engine."""


def handler(event, context):
    body = event["body"]
    if event.get("debug"):
        return {"debug": body}
    return {"user": body["user"]}
#@schema param handler.event: always={body}
#@schema param handler.event['body']: sometimes={user}
