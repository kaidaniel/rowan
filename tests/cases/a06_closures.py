def make_counter(fields):
    counts = {f: 0 for f in fields}

    def bump(name):
        counts[name] += 1

    return bump, (lambda: dict(counts))


bump, snap = make_counter(["clicks", "views"])
bump("clicks")                       #@ ok clicks
snap()["views"]                      #@ ok views
snap()["purchases"]                  #@ missing purchases
other, _ = make_counter(["purchases"])
other("purchases")                   #@ ok purchases
other("clicks")                      #@ missing clicks
bump("purchases")                    #@ missing purchases
