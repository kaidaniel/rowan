# os.environ: [] / get / setdefault / in; the environment is input (the driver sets it for determinism)
import os


def settings():  # APP_HOME was `input` (and always in the schema), but the driver sets it and later deletes it
    home = os.environ["APP_HOME"]                #@ ok APP_HOME ; missing APP_HOME
    level = os.environ.get("APP_LEVEL", "info")  #@ none APP_LEVEL
    os.environ.setdefault("APP_MODE", "dev")
    mode = os.environ["APP_MODE"]                #@ ok APP_MODE
    debug = "APP_DEBUG" in os.environ
    if debug:
        os.environ["APP_DEBUG"]                  #@ ok APP_DEBUG
    env = dict(os.environ)                       #@row env: definite={APP_HOME,APP_MODE} open
    out = {"home": home, "level": level, "mode": mode, "debug": debug}
    _ = 0                                        #@row out: present={home,level,mode,debug} definite={home,level,mode,debug} closed
    return out
# (no schema: the program provides APP_HOME itself)


if __name__ == "__main__":
    for k in ("APP_LEVEL", "APP_MODE", "APP_DEBUG"):
        os.environ.pop(k, None)
    os.environ["APP_HOME"] = "/srv"
    s1 = settings()
    m1 = s1["mode"]                              #@value m1: exactly={dev}
    os.environ.update({"APP_LEVEL": "debug", "APP_DEBUG": "1", "APP_MODE": "prod"})
    s2 = settings()
    m2 = s2["mode"]                              #@value m2: exactly={prod}
    del os.environ["APP_HOME"]
    try:
        settings()
    except KeyError:
        pass
