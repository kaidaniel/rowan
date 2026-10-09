# abc.ABC with an abstractmethod: the template method calls the subclass's implementation

from abc import ABC, abstractmethod


class Exporter(ABC):
    kind = "base"

    def export(self, rec):
        out = {"kind": self.kind}
        out.update(self.fields(rec))
        return out

    @abstractmethod
    def fields(self, rec): ...


class UserExporter(Exporter):
    kind = "user"

    def fields(self, rec):
        return {"name": rec["name"]}


class OrderExporter(Exporter):
    kind = "order"

    def fields(self, rec):
        return {"total": rec["qty"] * rec["price"], "id": rec["id"]}
#@schema param OrderExporter.fields.rec: always={qty,price,id} only={qty,price,id}


def export_all(exporter, recs):
    rows = [exporter.export(r) for r in recs]
    first = rows[0]
    #@row first: present={kind,name,total,id} definite={kind} closed
    return rows


u = UserExporter().export({"name": "ann", "age": 3})        #@row u: present={kind,name} definite={kind,name} closed
o = OrderExporter().export({"id": 1, "qty": 2, "price": 5})  #@row o: present={kind,total,id} definite={kind,total,id} closed
ot = o["total"]                                             #@value ot: exactly={10}
ok = o["kind"]                                              #@value ok: exactly={"order"}
export_all(UserExporter(), [{"name": "x"}])
export_all(OrderExporter(), [{"id": 2, "qty": 1, "price": 1, "note": ""}])
abstract = False
try:
    Exporter()
except TypeError:
    abstract = True
flag = abstract                                             #@value flag: exactly={True}
try:
    u["total"]                                              #@ missing total
except KeyError:
    pass
