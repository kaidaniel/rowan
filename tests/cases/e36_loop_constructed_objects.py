class K:
    def __init__(self, v):
        self.v = v

def f(k):
    return k.v                 #@ ok v

for i in range(20):
    k = K(i)
    k.v                        #@ ok v
    f(K(i))
    f(k)
