import json


def public(rec):
    return {k: v for k, v in rec.items() if k != "password"}


def shout(rec):
    return {k.upper(): v for k, v in rec.items()}


u = {"name": "k", "password": "…"}
p = public(u)                        #@row p: present={name} definite={name} closed
public(u)["password"]                #@ missing password
shout(public(u))["NAME"]             #@ ok NAME
shout(public(u))["name"]             #@ missing name
public(json.load(fp))["x"]           #@ input x
public(json.load(fp))["password"]    #@ missing password
#@schema json.load@L17: always={x}
