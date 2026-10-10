# json.loads of text read from a file is an open record: accesses are requirements on the input


def handle(body):
    import json
    msg = json.loads(body)
    user = msg["user"]                           #@ input user
    if msg.get("action") == "delete":            #@ none action
        return {"deleted": msg["id"]}            #@ input id
    _ = 0                                        #@row msg: definite={user} open
    return {"user": user["name"]}                #@ input name
#@schema json.loads@L6: always={user} sometimes={id} only={user,id}
#@schema json.loads@L6['user']: sometimes={name} only={name}


def serve(path):
    out = []
    with open(path) as f:
        for line in f:
            out.append(handle(line))
    return out


if __name__ == "__main__":
    res = serve("s043_requests.jsonl")
    n = len(res)                                 #@value n: unknown  (was 3, but the file is a run-time input the analysis never reads)
    r1 = res[1]                                  #@row r1: present={deleted,user} closed
