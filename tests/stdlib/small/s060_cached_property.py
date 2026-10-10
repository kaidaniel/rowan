# functools.cached_property: the first access computes the dict and stores it in the instance's __dict__

from functools import cached_property


class Report:
    def __init__(self, rows):
        self.rows = rows

    @cached_property
    def summary(self):
        total = sum(r["amount"] for r in self.rows)
        return {"count": len(self.rows), "total": total}


def check(rep):
    info = rep.summary
    #@row info: present={count,total} definite={count,total} closed
    info["count"]                              #@ ok count
    info["avg"]                                #@ missing avg


r = Report([{"amount": 2}, {"amount": 5, "id": 1}])
#@row r: present={rows} definite={rows} closed
s = r.summary                                  #@row s: present={count,total} definite={count,total} closed
#@row r: present={rows,summary} definite={rows,summary} closed
tot = s["total"]                               #@value tot: exactly={7}
again = r.summary is s                         #@value again: exactly={True}
#@todo schema param Report.__init__.rows['*']: always={amount} only={amount}  (a requirement made by a later method on a stored parameter: object-lifecycle schemas are not tracked)

if __name__ == "__main__":
    for rep in [r, Report([])]:
        try:
            check(rep)
        except KeyError:
            pass
