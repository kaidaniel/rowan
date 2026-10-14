"""A logging / writing / comparing method changes nothing it is given, also when its receiver
is the result of a chain of unknown calls (`log.getChild("y").info(...)`): the method's name
is the one written in the call. It was taken from the receiver's description, so such calls
havocked what they were given (sktime's `x.equals(y)`: 22k walks over a thousand objects)."""
import logging

log = logging.getLogger("x")
big = {"a": 1, "b": [1, 2]}
log.getChild("y").getChild("z").info("state %s", big)
a = big["a"]                                     #@ ok a
#@value a: exactly={1}
