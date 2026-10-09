# unbind removes a dim and gives a tuple as long as that dim; stacking the pieces back restores it
import torch


def unbind_rows(x):
    rows = x.unbind(0)
    r0 = rows[0]  #@shape r0: [*x.shape[1:]]
    cols = torch.unbind(x, dim=-1)
    c0 = cols[0]  #@shape c0: [*x.shape[:-1]]
    return r0, c0


def xyz(n):
    pts = torch.randn(n, 3)
    a, b, c = pts.unbind(1)  #@shape a: [n]; b: [n]; c: [n]
    #@requires none
    n_rows = len(pts.unbind(0))  #@dim n_rows: n
    return a, b, c, n_rows


def unbind_known(x):
    B, T, C = x.shape  #@requires x.dim() == 3
    steps = x.unbind(1)
    n = len(steps)  #@dim n: x.shape[1]
    s = steps[-1]  #@shape s: [x.shape[0], x.shape[2]]
    for t in x.unbind(1):
        u = t * 2  #@shape u: [x.shape[0], x.shape[2]]
    back = torch.stack(steps, dim=1)  #@shape back: [x.shape[0], x.shape[1], x.shape[2]]
    return s, back


def bad_dim():
    return torch.zeros(2, 3).unbind(2)  #@shape-error


if __name__ == "__main__":
    unbind_rows(torch.randn(3, 4))
    unbind_rows(torch.randn(2, 3, 5))
    xyz(4)
    unbind_known(torch.randn(2, 3, 4))
    try:
        bad_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
