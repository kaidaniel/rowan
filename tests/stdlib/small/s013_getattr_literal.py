# getattr / setattr / hasattr / delattr with literal names on instances whose attributes vary


class User:
    def __init__(self, name, email=None):
        self.name = name
        if email:
            self.email = email


def contact(u):
    if hasattr(u, "email"):
        return u.email                           #@ ok email
    return getattr(u, "phone", "n/a")            #@ none phone


def promote(u):
    setattr(u, "role", "admin")
    _ = 0                                        #@row u: present={name,email,role} definite={name,role} closed
    r = u.role                                   #@value r: exactly={admin}
    delattr(u, "role")
    _ = 0                                        #@row u: present={name,email} definite={name} absent={role} closed
    h = hasattr(u, "name")                       #@value h: exactly={True}
    return u


def must_email(u):
    return u.email                               #@ ok email ; missing email


if __name__ == "__main__":
    users = [User("a", "a@x"), User("b")]
    for u in users:
        contact(u)
        promote(u)
        try:
            must_email(u)
        except AttributeError:
            pass
    v = users[1]                                 #@row v: present={name} definite={name} closed
    try:
        v.role                                   #@ missing role
    except AttributeError:
        pass
