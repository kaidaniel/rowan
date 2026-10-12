class Base:
    def enrich(self, d):
        return d


class A(Base):
    def enrich(self, d):
        return {**d, "a": 1}


class B(Base):
    def enrich(self, d):
        return {**d, "b": 1}


def run(h: Base):
    return h.enrich({"id": 1})


run(A())["a"]                        #@ ok a
run(B())["a"]                        #@ missing a
run(B())["b"]                        #@ ok b
xs = [h.enrich({"id": 1}) for h in hs]  # (hs is unknown: its items need not be A / B — once taken as one of the classes defining enrich)
y = xs[0]                            #@row y: present={id} open
y["a"]                               #@ input a
y["id"]                              #@ input id
