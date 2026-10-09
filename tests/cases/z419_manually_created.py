def f(a, b, c):
    d = 1 + b  # d could be anything, because b.__add__ could be anything.  #@value d: unknown
    li = [a, b, 12, c, d]
    _ = li[-1]  # _ could be anything, because d could be anything.  #@value _: unknown
    _ = li[2]  # _ must be 12.  #@value _: exactly={12}
    x0, x1, x2, x3, x4 = li # x2 must be 12, x0, x1, x3, x4 could be anything.  #@value x2: exactly={12} ; x0: unknown ; x1: unknown ; x3: unknown ; x4: unknown
    _ = a(x2) # _ could be anything.  #@value _: unknown
    _ = a(b)(a) # _ could be anything.  #@value _: unknown
    if a:
        z = 1
    else:
        z = 100
    _ = z # _ is the join of 1 and 100. #@value _: exactly={1,100} ; _: includes={1,100}
    
