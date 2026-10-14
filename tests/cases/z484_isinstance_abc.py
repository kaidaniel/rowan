"""isinstance against collections.abc / typing classes: the builtin containers are instances
of the ABCs they implement (a tuple is an Iterable and a Sequence, not a Mapping); a value
whose class rowan cannot see through stays undecided. `_ntuple`-style helpers (timm) take
the iterable branch for a tuple."""
import collections.abc
import typing
from itertools import repeat


def to_pair(x):
    if isinstance(x, collections.abc.Iterable) and not isinstance(x, str):
        return tuple(x)
    return tuple(repeat(x, 2))


class Bag:
    def __iter__(self):
        return iter([1])


t_iter = isinstance((1, 2), collections.abc.Iterable)     #@value t_iter: exactly={True}
l_seq = isinstance([1], typing.Sequence)                  #@value l_seq: exactly={True}
d_map = isinstance({"a": 1}, collections.abc.Mapping)     #@value d_map: exactly={True}
t_map = isinstance((1,), collections.abc.Mapping)         #@value t_map: exactly={False}
s_seq = isinstance({1}, collections.abc.Sequence)         #@value s_seq: exactly={False}
b_iter = isinstance(Bag(), collections.abc.Iterable)      #@value b_iter: includes={True}
p = to_pair((1, None))
p0 = p[0]                                                 #@value p0: exactly={1}
p1 = p[1]                                                 #@value p1: exactly={None}
cls_str = isinstance(Bag, str)                            #@value cls_str: exactly={False}
cls_type = isinstance(Bag, type)                          #@value cls_type: exactly={True}
fn_type = isinstance(to_pair, type)                       #@value fn_type: exactly={False}
fn_tuple = isinstance(to_pair, (str, tuple))              #@value fn_tuple: exactly={False}
