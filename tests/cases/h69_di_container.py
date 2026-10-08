"""A service container: providers (classes) are stored by name, services receive the container
and resolve their dependencies in their constructors; one config dict is threaded through.

Expectations were written from the program text before running the engine."""


class Container:
    def __init__(self, config):
        self.config = config
        self.providers = {}
        self.instances = {}

    def register(self, name, provider):
        self.providers[name] = provider

    def resolve(self, name):
        if name in self.instances:
            return self.instances[name]
        inst = self.providers[name](self)
        self.instances[name] = inst
        return inst


class Database:
    def __init__(self, c):
        cfg = c.config["db"]
        self.dsn = {"host": cfg["host"], "port": cfg.get("port", 5432)}
        self.pool = []

    def connect(self):
        conn = {"dsn": self.dsn, "open": True}
        self.pool.append(conn)
        return conn


class Cache:
    def __init__(self, c):
        self.ttl = c.config.get("cache_ttl", 60)
        self.store = {}

    def put(self, k, v):
        self.store[k] = v


class Mailer:
    def __init__(self, c):
        self.sent = []

    def verify(self, u):
        self.sent.append({"to": u["id"], "kind": "verify"})
        return True


class UserRepo:
    def __init__(self, c):
        self.db = c.resolve("db")
        self.cache = c.resolve("cache")

    def find(self, uid):
        conn = self.db.connect()
        user = {"id": uid, "conn": conn}
        self.cache.put("user", user)
        return user


class UserService:
    def __init__(self, c):
        self.repo = c.resolve("users")
        self.mailer = c.resolve("mailer")

    def profile(self, uid):
        u = self.repo.find(uid)
        p = {"user": u, "email_ok": self.mailer.verify(u)}
        if self.repo.cache.ttl:
            p["cached"] = True
        return p


def build(config):
    c = Container(config)
    c.register("db", Database)
    c.register("cache", Cache)
    c.register("users", UserRepo)
    c.register("mailer", Mailer)
    c.register("service", UserService)
    return c


c = build({"db": {"host": "h"}, "cache_ttl": 0})
_ = 0                                     #@row c: present={config,providers,instances} definite={config,providers,instances} closed
providers = c.providers                   #@row providers: present={db,cache,users,mailer,service} definite={db,cache,users,mailer,service} closed
svc = c.resolve("service")                #@row svc: present={repo,mailer} definite={repo,mailer} closed
repo = svc.repo                           #@row repo: present={db,cache} definite={db,cache} closed
db = repo.db                              #@row db: present={dsn,pool} definite={dsn,pool} closed
dsn = db.dsn                              #@row dsn: present={host,port} definite={host,port} closed
inst = c.instances                        #@row inst: present={db,cache,users,mailer,service} definite={db,cache,users,mailer,service} closed
# ttl is 0 for this container, so "cached" is never set.
prof = svc.profile(7)                     #@row prof: present={user,email_ok} definite={user,email_ok} closed
user = prof["user"]                       #@row user: present={id,conn} definite={id,conn} closed
conn = user["conn"]                       #@row conn: present={dsn,open} definite={dsn,open} closed
cache_store = repo.cache.store            #@row cache_store: present={user} definite={user} closed
sent = svc.mailer.sent[0]                 #@row sent: present={to,kind} definite={to,kind} closed
pooled = db.pool[0]                       #@row pooled: present={dsn,open} definite={dsn,open} closed
same = c.resolve("service")               #@row same: present={repo,mailer} definite={repo,mailer} closed

# A second container with a non-zero ttl.
c2 = build({"db": {"host": "h", "port": 1}, "cache_ttl": 30})
prof2 = c2.resolve("service").profile(8)  #@row prof2: present={user,email_ok,cached} definite={user,email_ok,cached} closed
dsn2 = c2.resolve("db").dsn               #@row dsn2: present={host,port} definite={host,port} closed
prof2["cached"]                           #@ ok cached
prof["cached"]                            #@ missing cached
