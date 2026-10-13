def _call_aside(f, *args, **kwargs):
    f(*args, **kwargs)
    return f


known = 1


@_call_aside
def _initialize():
    working_set = ["master"]
    globals().update(locals())
