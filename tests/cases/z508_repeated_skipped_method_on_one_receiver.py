# The same method past its budget called again and again on one receiver: what the receiver
# holds under the names the method reads is walked as unknown code once, then only where it
# changed since (the walks of a skipped method's receiver were not remembered: click walked
# 17M objects, 4.7M now).
#@limits budget=3
#@havoc <= 80


class Model:
    def __init__(self):
        self.params = {"k%d" % i: {"v": i} for i in range(30)}

    def get(self, k):
        a = 1
        b = 2
        return self.params.get(k)


m = Model()
for i in range(10):
    m.get("k1")
x = m.params["k2"]  #@ ok params ; sound k2
