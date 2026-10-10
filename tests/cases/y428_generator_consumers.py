"""Hypothesis: a generator is one-shot: a second loop over it sees nothing; a loop that breaks
leaves the rest for next(); list(zip(g, "ab")) takes three items from g, zip("ab", g) two;
islice(g, 3) takes three; a function that takes two items with next() leaves the rest; a
generator that a loop uses up has had all its effects."""
import itertools


def numbers(n, log):
    for i in range(n):
        log.append(i)
        yield i


def take_two(it):
    return [next(it), next(it)]


log = []
g = numbers(5, log)
first = [x for x in g]
again = [x for x in g]
na = len(again)                                  #@value na: exactly={0}
nl = len(log)                                    #@value nl: exactly={5}
g2 = numbers(5, [])
for x in g2:
    if x == 1:
        break
rest = next(g2)                                  #@value rest: exactly={2}
g3 = numbers(5, [])
z = list(zip(g3, "ab"))
after = next(g3)                                 #@value after: exactly={3}
g4 = numbers(5, [])
z2 = list(zip("ab", g4))
after2 = next(g4)                                #@value after2: exactly={2}
g5 = numbers(5, [])
s = list(itertools.islice(g5, 3))
after3 = next(g5)                                #@value after3: exactly={3}
log6 = []
g6 = numbers(5, log6)
two = take_two(g6)
after4 = next(g6)                                #@value after4: exactly={2}
n6 = len(log6)                                   #@value n6: exactly={3}
