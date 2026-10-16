"""A method of a base class with very many subclasses (more than 64), run for any call: its
receiver is the base's instance made by its `__init__`, or a subclass's with any attributes.
Running every subclass's `__init__` for it (sktime's `BaseObject`, 1,089 subclasses) made each
such method's run on demand take seconds. (The whole program's runs still make each
subclass's instance with its defaults: about 2 runs per subclass, its entry run 2 more.)"""
#@tabulations <= 600


class Estimator:
    def __init__(self):
        self.name = "base"
        self.fitted = False

    def describe(self):
        n = self.name
#@general n: includes={"base"} unknown
        return n


class Model0(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m0"
        self.alpha = alpha


class Model1(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m1"
        self.alpha = alpha


class Model2(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m2"
        self.alpha = alpha


class Model3(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m3"
        self.alpha = alpha


class Model4(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m4"
        self.alpha = alpha


class Model5(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m5"
        self.alpha = alpha


class Model6(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m6"
        self.alpha = alpha


class Model7(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m7"
        self.alpha = alpha


class Model8(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m8"
        self.alpha = alpha


class Model9(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m9"
        self.alpha = alpha


class Model10(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m10"
        self.alpha = alpha


class Model11(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m11"
        self.alpha = alpha


class Model12(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m12"
        self.alpha = alpha


class Model13(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m13"
        self.alpha = alpha


class Model14(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m14"
        self.alpha = alpha


class Model15(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m15"
        self.alpha = alpha


class Model16(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m16"
        self.alpha = alpha


class Model17(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m17"
        self.alpha = alpha


class Model18(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m18"
        self.alpha = alpha


class Model19(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m19"
        self.alpha = alpha


class Model20(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m20"
        self.alpha = alpha


class Model21(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m21"
        self.alpha = alpha


class Model22(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m22"
        self.alpha = alpha


class Model23(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m23"
        self.alpha = alpha


class Model24(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m24"
        self.alpha = alpha


class Model25(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m25"
        self.alpha = alpha


class Model26(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m26"
        self.alpha = alpha


class Model27(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m27"
        self.alpha = alpha


class Model28(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m28"
        self.alpha = alpha


class Model29(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m29"
        self.alpha = alpha


class Model30(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m30"
        self.alpha = alpha


class Model31(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m31"
        self.alpha = alpha


class Model32(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m32"
        self.alpha = alpha


class Model33(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m33"
        self.alpha = alpha


class Model34(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m34"
        self.alpha = alpha


class Model35(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m35"
        self.alpha = alpha


class Model36(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m36"
        self.alpha = alpha


class Model37(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m37"
        self.alpha = alpha


class Model38(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m38"
        self.alpha = alpha


class Model39(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m39"
        self.alpha = alpha


class Model40(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m40"
        self.alpha = alpha


class Model41(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m41"
        self.alpha = alpha


class Model42(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m42"
        self.alpha = alpha


class Model43(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m43"
        self.alpha = alpha


class Model44(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m44"
        self.alpha = alpha


class Model45(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m45"
        self.alpha = alpha


class Model46(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m46"
        self.alpha = alpha


class Model47(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m47"
        self.alpha = alpha


class Model48(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m48"
        self.alpha = alpha


class Model49(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m49"
        self.alpha = alpha


class Model50(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m50"
        self.alpha = alpha


class Model51(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m51"
        self.alpha = alpha


class Model52(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m52"
        self.alpha = alpha


class Model53(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m53"
        self.alpha = alpha


class Model54(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m54"
        self.alpha = alpha


class Model55(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m55"
        self.alpha = alpha


class Model56(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m56"
        self.alpha = alpha


class Model57(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m57"
        self.alpha = alpha


class Model58(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m58"
        self.alpha = alpha


class Model59(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m59"
        self.alpha = alpha


class Model60(Estimator):
    def __init__(self, alpha=0.0):
        super().__init__()
        self.name = "m60"
        self.alpha = alpha


class Model61(Estimator):
    def __init__(self, alpha=0.1):
        super().__init__()
        self.name = "m61"
        self.alpha = alpha


class Model62(Estimator):
    def __init__(self, alpha=0.2):
        super().__init__()
        self.name = "m62"
        self.alpha = alpha


class Model63(Estimator):
    def __init__(self, alpha=0.3):
        super().__init__()
        self.name = "m63"
        self.alpha = alpha


class Model64(Estimator):
    def __init__(self, alpha=0.4):
        super().__init__()
        self.name = "m64"
        self.alpha = alpha


class Model65(Estimator):
    def __init__(self, alpha=0.5):
        super().__init__()
        self.name = "m65"
        self.alpha = alpha


class Model66(Estimator):
    def __init__(self, alpha=0.6):
        super().__init__()
        self.name = "m66"
        self.alpha = alpha


class Model67(Estimator):
    def __init__(self, alpha=0.7):
        super().__init__()
        self.name = "m67"
        self.alpha = alpha


class Model68(Estimator):
    def __init__(self, alpha=0.8):
        super().__init__()
        self.name = "m68"
        self.alpha = alpha


class Model69(Estimator):
    def __init__(self, alpha=0.9):
        super().__init__()
        self.name = "m69"
        self.alpha = alpha


first = Model0().describe()
#@value first: exactly={"m0"}
