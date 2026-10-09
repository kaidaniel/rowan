"""Hypothesis: an exception instance raised with a record attribute reaches the handler's
`as` name, so the handler can read the record.

Expectations were written from the program text before running the engine."""


class ValidationError(Exception):
    def __init__(self, info):
        super().__init__("bad")
        self.info = info


def check(rec):
    if "id" not in rec:
        raise ValidationError({"field": "id", "rec": rec})
    return rec


try:
    check({"name": "x"})
    res = {"ok": True}
except ValidationError as e:
    res = e.info
_ = 0                                        #@row res: present={field,rec} definite={field,rec} closed
