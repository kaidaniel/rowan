DEFAULTS = {"timeout": 10, "retries": 3}
registry = {}


def register(name, value):
    registry[name] = value


def build(user):
    return {**DEFAULTS, **user, "built": True}


class Config:
    def __init__(self, data):
        self.data = data

    def get(self, k):
        return self.data[k]
