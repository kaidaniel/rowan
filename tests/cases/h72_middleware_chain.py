"""Middleware chain: each middleware wraps the next handler (closures over closures, one made by
a factory), request and response dicts are threaded through and enriched at each layer.

Expectations were written from the program text before running the engine."""
import sys


def logging_mw(nxt):
    def handler(req):
        req["log"] = []
        resp = nxt(req)
        resp["logged"] = True
        return resp
    return handler


def auth_mw(users):
    def mw(nxt):
        def handler(req):
            token = req["headers"].get("token")
            if token in users:
                req["user"] = users[token]
                return nxt(req)
            return {"status": 401, "error": "no"}
        return handler
    return mw


def json_mw(nxt):
    def handler(req):
        resp = nxt(req)
        resp["body"] = {"data": resp.pop("data", None)}
        resp["content_type"] = "json"
        return resp
    return handler


def app(req):
    return {"status": 200, "data": {"user": req["user"]["name"], "path": req["path"]}}


def build(middlewares, endpoint):
    h = endpoint
    for mw in reversed(middlewares):
        h = mw(h)
    return h


USERS = {"t1": {"name": "ann", "role": "admin"}}
handler = build([logging_mw, auth_mw(USERS), json_mw], app)

# Known token: logging -> auth -> json -> app.
req = {"path": "/", "headers": {"token": "t1"}}
ok = handler(req)                         #@row ok: present={status,body,content_type,logged} definite={status,body,content_type,logged} closed
body = ok["body"]                         #@row body: present={data} definite={data} closed
data = body["data"]                       #@row data: present={user,path} definite={user,path} closed
_ = 0                                     #@row req: present={path,headers,log,user} definite={path,headers,log,user} closed
ok["data"]                                #@ missing data

# No token: auth short-circuits, json_mw never runs, logging still marks the response.
denied = handler({"path": "/x", "headers": {}})  #@row denied: present={status,error,logged} definite={status,error,logged} closed
denied["body"]                            #@ missing body

# Token from the command line: either outcome.  "logged" is set on whichever response comes
# back, but the store goes through a pointer to two objects and is weak on each (a known
# imprecision), so only "status" is definite in the joined row.
maybe = handler({"path": "/", "headers": {"token": sys.argv[1]}})  #@row maybe: present={status,body,content_type,logged,error} definite={status} closed
maybe["error"]                            #@ possibly error

# A shorter chain without auth: app sees no "user" key.
plain = build([json_mw], app)
resp = plain({"path": "/p", "headers": {}, "user": {"name": "bob"}})  #@row resp: present={status,body,content_type} definite={status,body,content_type} closed
resp["logged"]                            #@ missing logged
