VERSION = {"major": 2, "minor": 0}


class Record:
    def __init__(self, name, fields):
        self.name = name
        self.fields = dict(fields)
        self.meta = {"source": "tinylib"}

    def with_field(self, key, value):
        self.fields[key] = value
        return self

    def to_dict(self):
        d = {"name": self.name, "fields": dict(self.fields)}
        if self.meta:
            d["meta"] = dict(self.meta)
        return d


def load(name):
    return Record(name, {"id": 1}).with_field("loaded", True)
