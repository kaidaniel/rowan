"""A Counter of an input's items is open: a key not known to be absent may already have any
count, so reading it is not just the missing-key 0."""
import collections
import json
import sys

levels = collections.Counter(json.loads(sys.argv[1]))
n = levels["ERROR"]                          #@value n: includes=0
_ = n                                        #@value n: unknown
