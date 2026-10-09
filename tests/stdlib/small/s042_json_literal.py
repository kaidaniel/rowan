# json.loads of literal text is a known record; dumps/loads round trips (int keys become strings); object_hook
import json

CONFIG = json.loads('{"name": "svc", "db": {"host": "h", "port": 5432}, "tags": ["a"]}')


def lower_keys(d):
    return {k.lower(): v for k, v in d.items()}


def parse(text):
    return json.loads(text, object_hook=lower_keys)


def roundtrip(rec):
    return json.loads(json.dumps(rec))


if __name__ == "__main__":
    c = CONFIG                                   #@row c: present={name,db,tags} definite={name,db,tags} closed
    db = CONFIG["db"]                            #@row db: present={host,port} definite={host,port} closed
    port = db["port"]                            #@value port: exactly={5432}
    try:
        CONFIG["version"]                        #@ missing version
    except KeyError:
        pass
    r = roundtrip({"a": 1, "b": None})           #@row r: present={a,b} definite={a,b} closed
    b = r["b"]                                   #@value b: exactly={None}
    s = roundtrip({1: "x"})                      #@row s: present={"1"} definite={"1"} absent={1} closed
    p1 = parse('{"ID": 1, "Inner": {"X": 2}}')   #@row p1: present={id,inner} definite={id,inner} closed
    inner = p1["inner"]                          #@row inner: present={x} definite={x} closed
    p2 = parse('{"Name": "n"}')                  #@row p2: present={name} definite={name} closed
    n = len(json.dumps({"a": 1}))                #@value n: exactly={8}
