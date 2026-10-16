# Floats are shown as Python's repr shows them: `1e+16`, not `10000000000000000` (which reads
# as an int), `-1e-07`, `3.3333333333333334e-08`.
f = 1e16                                     #@value f: exactly={1e16}
g = -1e-7                                    #@value g: exactly={-1e-07}
h = 1 / 3e7                                  #@value h: exactly={3.3333333333333334e-08}
k = f * 2                                    #@value k: exactly={2e16}
m = 2.5                                      #@value m: exactly={2.5}
