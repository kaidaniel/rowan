class Record:
    def __init__(self, data):
        self.data = data

    def __getitem__(self, key):
        return self.data[key]

    def __contains__(self, key):
        return key in self.data


r = Record({"a": 1, "b": 2})
r["a"]                                          #@ ok a
r["zz"]                                         #@ missing zz
# __contains__ narrows self.data, which is closed without "c": the body is dead code
if "c" in r:
    r["c"]                                      #@ none c
