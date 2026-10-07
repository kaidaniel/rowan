import functools

CACHE = {}


def put(k, v):
    CACHE[k] = v


def log(fn):
    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        print("calling", fn.__name__)
        return fn(*args, **kwargs)
    return wrapper


@log
def make(x):
    return {"x": x, "src": "make"}


def with_opts(opts=None):
    opts = opts or {}
    opts.setdefault("retries", 3)
    return opts


put("a", 1)
CACHE["a"]                                          #@ ok a
CACHE["b"]                                          #@ missing b
make(1)["src"]                                      #@ ok src
make(1)["y"]                                        #@ missing y
with_opts()["retries"]                              #@ ok retries
with_opts({"y": 2})["y"]                            #@ ok y
with_opts({"y": 2})["z"]                            #@ missing z
