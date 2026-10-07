class Cache:
    def __init__(self):
        self.store = {}

    def put(self, k, v):
        self.store[k] = v

    def user(self):
        return self.store["user"]


c = Cache()
c.user()                             #@ missing user
c.put("user", u)
c.user()                             #@ ok user
d = Cache()
d.user()                             #@ missing user
c.user()                             #@ ok user
