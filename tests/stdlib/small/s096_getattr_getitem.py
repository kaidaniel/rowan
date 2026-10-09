# user classes with __getattr__ / __getitem__ / __contains__ over a backing dict: reads map to its keys


class Record:
    def __init__(self, data):
        self._data = data

    def __getattr__(self, name):
        try:
            return self._data[name]
        except KeyError:
            raise AttributeError(f"no field {name!r}", name=name, obj=self) from None

    def __getitem__(self, key):
        return self._data[key]

    def __contains__(self, key):
        return key in self._data


def contact(rec):
    r = Record(rec)
    name = r.name                                 #@ ok name
    mail = r["email"] if "email" in r else None
    phone = r.phone                               #@ possibly phone
    return {"name": name, "mail": mail, "phone": phone}
#@schema param contact.rec: always={name,phone} only={name,phone}


c = contact({"name": "a", "phone": "1"})          #@row c: present={name,mail,phone} definite={name,mail,phone} closed
cm = c["mail"]                                    #@value cm: exactly={None}
try:
    contact({"name": "b", "email": "e"})
except AttributeError:
    pass
r = Record({"x": 1})                              #@row r: present={_data} definite={_data} closed
x = r.x                                           #@value x: exactly={1}
xi = r["x"]                                       #@value xi: exactly={1}
try:
    r["y"]                                        #@ missing y
except KeyError:
    pass
