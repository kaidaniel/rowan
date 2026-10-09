# str / list / dict values in a torch module keep their meaning (split, sort, get, index, len)
import torch


def from_text():
    parts = "2,3,4".split(",")  #@shape parts: none
    dims = [int(p) for p in parts]  #@shape dims: none
    dims.sort(reverse=True)
    n = len(dims)  #@dim n: 3
    t = torch.zeros(*dims)  #@shape t: [4, 3, 2]
    cfg = {"hidden": 8, "layers": 2}
    h = cfg.get("hidden")  #@dim h: 8
    k = cfg.get("missing", 5)  #@dim k: 5
    i = "abc".index("c")  #@dim i: 2
    u = torch.zeros(h, k, i)  #@shape u: [8, 5, 2]
    names = sorted(["b", "a", "c"])  #@shape names: none
    v = torch.zeros(len(names), n)  #@shape v: [3, 3]
    return t, u, v


def from_line(line, x):
    B, C = x.shape
    parts = line.split(",")  #@shape parts: none
    head = parts[0]  #@shape head: none
    w = torch.zeros(len(parts), C)  #@shape w: [_, x.shape[1]]
    y = x.view(B, C)  #@shape y: [x.shape[0], x.shape[1]]
    return w, y


def from_dict(cfg, x):
    B, T = x.shape
    d = cfg.get("d", 4)
    e = torch.zeros(B, d)  #@shape e: [x.shape[0], _]
    keys = list(cfg.keys())  #@shape keys: none
    return e


if __name__ == "__main__":
    t, u, v = from_text()  #@shape t: [4, 3, 2]; u: [8, 5, 2]; v: [3, 3]
    w, y = from_line("a,b,c", torch.randn(2, 5))  #@shape w: [3, 5]; y: [2, 5]
    e = from_dict({"d": 6}, torch.randn(3, 2))  #@shape e: [3, 6]
    e2 = from_dict({}, torch.randn(3, 2))  #@shape e2: [3, 4]
