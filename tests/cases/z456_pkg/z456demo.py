STATE = {"ran": False}


def demo():
    STATE["ran"] = True


if __name__ == "__main__":
    demo()
    imported = False
else:
    imported = True
