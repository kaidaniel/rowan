# split / partition / join / replace / startswith / strip parsing text into records


def parse_kv(text):
    out = {}
    for part in text.split(";"):
        k, sep, v = part.partition("=")
        if sep:
            out[k.strip()] = v.strip()
    return out


def auth(headers):
    tok = headers.get("Authorization", "")
    if tok.startswith("Bearer "):
        return {"token": tok.removeprefix("Bearer "), "scheme": "bearer"}
    return {"scheme": "none"}


if __name__ == "__main__":
    c = parse_kv("host = a; port=1; junk")       #@row c: present={host,port} definite={host,port} closed
    h = c["host"]                                #@value h: exactly={"a"}
    e = parse_kv("")                             #@row e: present={} closed
    a1 = auth({"Authorization": "Bearer xyz"})   #@row a1: present={token,scheme} definite={token,scheme} closed
    t = a1["token"]                              #@value t: exactly={"xyz"}
    a2 = auth({})                                #@row a2: present={scheme} definite={scheme} closed
    try:
        a2["token"]                              #@ missing token
    except KeyError:
        pass
    j = "-".join(["a", "b"]).replace("-", "_").upper()  #@value j: exactly={"A_B"}
    head, rest = "k:v:w".split(":", 1)
    #@value rest: exactly={"v:w"}
    base = "dir/file.txt".rpartition("/")[2].strip(".txt")  #@value base: exactly={"file"}
