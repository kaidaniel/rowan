def build(user, admin):
    d = {"id": user.id}
    if admin:
        d["role"] = "admin"
    return d


r = build(u, flag)                   #@row r: present={id,role} definite={id} closed
r["role"]                            #@ possibly role
if "role" in r:
    r["role"]                        #@ ok role
r.get("role", "user")                #@ none role
r["rolle"]                           #@ missing rolle
try:
    r["tenant"]                      #@ none tenant
except KeyError:
    pass
r["id"]                              #@ ok id
#@schema name u: always={id}
