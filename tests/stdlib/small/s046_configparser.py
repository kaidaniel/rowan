# configparser over literal text: sections, options (lower-cased), DEFAULT inheritance, getint, fallback
import configparser

INI = """
[DEFAULT]
timeout = 30

[server]
Host = example.org
port = 8080

[client]
retries = 3
"""

OTHER = """
[server]
host = localhost
"""


def load(text):
    cp = configparser.ConfigParser()
    cp.read_string(text)
    return cp


def endpoint(sec):
    host = sec["host"]                           #@ ok host
    port = sec.getint("port", fallback=80)
    return f"{host}:{port}"


if __name__ == "__main__":
    cp = load(INI)
    secs = tuple(cp.sections())                  #@value secs: exactly={("server","client")}
    port = cp.getint("server", "port")           #@value port: exactly={8080}
    t = cp["client"].getint("timeout")           #@value t: exactly={30}
    h = cp.get("client", "host", fallback="none")  #@value h: exactly={none}
    server = cp["server"]                        #@row server: present={host,port,timeout} definite={host,port,timeout} absent={Host} closed
    plain = dict(cp["client"])                   #@row plain: present={retries,timeout} definite={retries,timeout} closed
    try:
        cp["db"]                                 #@ missing db
    except KeyError:
        pass
    try:
        cp["server"]["user"]                     #@ missing user
    except KeyError:
        pass
    e1 = endpoint(cp["server"])                  #@value e1: exactly={"example.org:8080"}
    e2 = endpoint(load(OTHER)["server"])         #@value e2: exactly={"localhost:80"}
