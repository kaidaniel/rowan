"""A fake installed package for z717_lib_closure.py: its import spends an entry's allowance
(`expensive`), then defines 40 subclasses of `Base`, whose `__init_subclass__` hook does not
run past the allowance. The hook's code reaches 200 helpers (`h000` … `h199`, a chain), so each
of those calls that do not run may change what that code reaches: `REGISTRY`, `TABLE` and
`STATE`. The hook's own loop needs more than an entry's allowance too, so a subclass the program
defines runs the hook, but not its call of `h000`."""

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
REGISTRY = {}
STATE = {"n": "none"}
CONST = {"a": 1}


def _cell(i, j):
    return i * j


def expensive():
    out = {}
    for i in ROWS:
        for j in ROWS:
            out[i] = _cell(i, j)
    return out


TABLE = expensive()


def h000(c):
    if c is None:
        return None
    return h001(c)


def h001(c):
    if c is None:
        return None
    return h002(c)


def h002(c):
    if c is None:
        return None
    return h003(c)


def h003(c):
    if c is None:
        return None
    return h004(c)


def h004(c):
    if c is None:
        return None
    return h005(c)


def h005(c):
    if c is None:
        return None
    return h006(c)


def h006(c):
    if c is None:
        return None
    return h007(c)


def h007(c):
    if c is None:
        return None
    return h008(c)


def h008(c):
    if c is None:
        return None
    return h009(c)


def h009(c):
    if c is None:
        return None
    return h010(c)


def h010(c):
    if c is None:
        return None
    return h011(c)


def h011(c):
    if c is None:
        return None
    return h012(c)


def h012(c):
    if c is None:
        return None
    return h013(c)


def h013(c):
    if c is None:
        return None
    return h014(c)


def h014(c):
    if c is None:
        return None
    return h015(c)


def h015(c):
    if c is None:
        return None
    return h016(c)


def h016(c):
    if c is None:
        return None
    return h017(c)


def h017(c):
    if c is None:
        return None
    return h018(c)


def h018(c):
    if c is None:
        return None
    return h019(c)


def h019(c):
    if c is None:
        return None
    return h020(c)


def h020(c):
    if c is None:
        return None
    return h021(c)


def h021(c):
    if c is None:
        return None
    return h022(c)


def h022(c):
    if c is None:
        return None
    return h023(c)


def h023(c):
    if c is None:
        return None
    return h024(c)


def h024(c):
    if c is None:
        return None
    return h025(c)


def h025(c):
    if c is None:
        return None
    return h026(c)


def h026(c):
    if c is None:
        return None
    return h027(c)


def h027(c):
    if c is None:
        return None
    return h028(c)


def h028(c):
    if c is None:
        return None
    return h029(c)


def h029(c):
    if c is None:
        return None
    return h030(c)


def h030(c):
    if c is None:
        return None
    return h031(c)


def h031(c):
    if c is None:
        return None
    return h032(c)


def h032(c):
    if c is None:
        return None
    return h033(c)


def h033(c):
    if c is None:
        return None
    return h034(c)


def h034(c):
    if c is None:
        return None
    return h035(c)


def h035(c):
    if c is None:
        return None
    return h036(c)


def h036(c):
    if c is None:
        return None
    return h037(c)


def h037(c):
    if c is None:
        return None
    return h038(c)


def h038(c):
    if c is None:
        return None
    return h039(c)


def h039(c):
    if c is None:
        return None
    return h040(c)


def h040(c):
    if c is None:
        return None
    return h041(c)


def h041(c):
    if c is None:
        return None
    return h042(c)


def h042(c):
    if c is None:
        return None
    return h043(c)


def h043(c):
    if c is None:
        return None
    return h044(c)


def h044(c):
    if c is None:
        return None
    return h045(c)


def h045(c):
    if c is None:
        return None
    return h046(c)


def h046(c):
    if c is None:
        return None
    return h047(c)


def h047(c):
    if c is None:
        return None
    return h048(c)


def h048(c):
    if c is None:
        return None
    return h049(c)


def h049(c):
    if c is None:
        return None
    return h050(c)


def h050(c):
    if c is None:
        return None
    return h051(c)


def h051(c):
    if c is None:
        return None
    return h052(c)


def h052(c):
    if c is None:
        return None
    return h053(c)


def h053(c):
    if c is None:
        return None
    return h054(c)


def h054(c):
    if c is None:
        return None
    return h055(c)


def h055(c):
    if c is None:
        return None
    return h056(c)


def h056(c):
    if c is None:
        return None
    return h057(c)


def h057(c):
    if c is None:
        return None
    return h058(c)


def h058(c):
    if c is None:
        return None
    return h059(c)


def h059(c):
    if c is None:
        return None
    return h060(c)


def h060(c):
    if c is None:
        return None
    return h061(c)


def h061(c):
    if c is None:
        return None
    return h062(c)


def h062(c):
    if c is None:
        return None
    return h063(c)


def h063(c):
    if c is None:
        return None
    return h064(c)


def h064(c):
    if c is None:
        return None
    return h065(c)


def h065(c):
    if c is None:
        return None
    return h066(c)


def h066(c):
    if c is None:
        return None
    return h067(c)


def h067(c):
    if c is None:
        return None
    return h068(c)


def h068(c):
    if c is None:
        return None
    return h069(c)


def h069(c):
    if c is None:
        return None
    return h070(c)


def h070(c):
    if c is None:
        return None
    return h071(c)


def h071(c):
    if c is None:
        return None
    return h072(c)


def h072(c):
    if c is None:
        return None
    return h073(c)


def h073(c):
    if c is None:
        return None
    return h074(c)


def h074(c):
    if c is None:
        return None
    return h075(c)


def h075(c):
    if c is None:
        return None
    return h076(c)


def h076(c):
    if c is None:
        return None
    return h077(c)


def h077(c):
    if c is None:
        return None
    return h078(c)


def h078(c):
    if c is None:
        return None
    return h079(c)


def h079(c):
    if c is None:
        return None
    return h080(c)


def h080(c):
    if c is None:
        return None
    return h081(c)


def h081(c):
    if c is None:
        return None
    return h082(c)


def h082(c):
    if c is None:
        return None
    return h083(c)


def h083(c):
    if c is None:
        return None
    return h084(c)


def h084(c):
    if c is None:
        return None
    return h085(c)


def h085(c):
    if c is None:
        return None
    return h086(c)


def h086(c):
    if c is None:
        return None
    return h087(c)


def h087(c):
    if c is None:
        return None
    return h088(c)


def h088(c):
    if c is None:
        return None
    return h089(c)


def h089(c):
    if c is None:
        return None
    return h090(c)


def h090(c):
    if c is None:
        return None
    return h091(c)


def h091(c):
    if c is None:
        return None
    return h092(c)


def h092(c):
    if c is None:
        return None
    return h093(c)


def h093(c):
    if c is None:
        return None
    return h094(c)


def h094(c):
    if c is None:
        return None
    return h095(c)


def h095(c):
    if c is None:
        return None
    return h096(c)


def h096(c):
    if c is None:
        return None
    return h097(c)


def h097(c):
    if c is None:
        return None
    return h098(c)


def h098(c):
    if c is None:
        return None
    return h099(c)


def h099(c):
    if c is None:
        return None
    return h100(c)


def h100(c):
    if c is None:
        return None
    return h101(c)


def h101(c):
    if c is None:
        return None
    return h102(c)


def h102(c):
    if c is None:
        return None
    return h103(c)


def h103(c):
    if c is None:
        return None
    return h104(c)


def h104(c):
    if c is None:
        return None
    return h105(c)


def h105(c):
    if c is None:
        return None
    return h106(c)


def h106(c):
    if c is None:
        return None
    return h107(c)


def h107(c):
    if c is None:
        return None
    return h108(c)


def h108(c):
    if c is None:
        return None
    return h109(c)


def h109(c):
    if c is None:
        return None
    return h110(c)


def h110(c):
    if c is None:
        return None
    return h111(c)


def h111(c):
    if c is None:
        return None
    return h112(c)


def h112(c):
    if c is None:
        return None
    return h113(c)


def h113(c):
    if c is None:
        return None
    return h114(c)


def h114(c):
    if c is None:
        return None
    return h115(c)


def h115(c):
    if c is None:
        return None
    return h116(c)


def h116(c):
    if c is None:
        return None
    return h117(c)


def h117(c):
    if c is None:
        return None
    return h118(c)


def h118(c):
    if c is None:
        return None
    return h119(c)


def h119(c):
    if c is None:
        return None
    return h120(c)


def h120(c):
    if c is None:
        return None
    return h121(c)


def h121(c):
    if c is None:
        return None
    return h122(c)


def h122(c):
    if c is None:
        return None
    return h123(c)


def h123(c):
    if c is None:
        return None
    return h124(c)


def h124(c):
    if c is None:
        return None
    return h125(c)


def h125(c):
    if c is None:
        return None
    return h126(c)


def h126(c):
    if c is None:
        return None
    return h127(c)


def h127(c):
    if c is None:
        return None
    return h128(c)


def h128(c):
    if c is None:
        return None
    return h129(c)


def h129(c):
    if c is None:
        return None
    return h130(c)


def h130(c):
    if c is None:
        return None
    return h131(c)


def h131(c):
    if c is None:
        return None
    return h132(c)


def h132(c):
    if c is None:
        return None
    return h133(c)


def h133(c):
    if c is None:
        return None
    return h134(c)


def h134(c):
    if c is None:
        return None
    return h135(c)


def h135(c):
    if c is None:
        return None
    return h136(c)


def h136(c):
    if c is None:
        return None
    return h137(c)


def h137(c):
    if c is None:
        return None
    return h138(c)


def h138(c):
    if c is None:
        return None
    return h139(c)


def h139(c):
    if c is None:
        return None
    return h140(c)


def h140(c):
    if c is None:
        return None
    return h141(c)


def h141(c):
    if c is None:
        return None
    return h142(c)


def h142(c):
    if c is None:
        return None
    return h143(c)


def h143(c):
    if c is None:
        return None
    return h144(c)


def h144(c):
    if c is None:
        return None
    return h145(c)


def h145(c):
    if c is None:
        return None
    return h146(c)


def h146(c):
    if c is None:
        return None
    return h147(c)


def h147(c):
    if c is None:
        return None
    return h148(c)


def h148(c):
    if c is None:
        return None
    return h149(c)


def h149(c):
    if c is None:
        return None
    return h150(c)


def h150(c):
    if c is None:
        return None
    return h151(c)


def h151(c):
    if c is None:
        return None
    return h152(c)


def h152(c):
    if c is None:
        return None
    return h153(c)


def h153(c):
    if c is None:
        return None
    return h154(c)


def h154(c):
    if c is None:
        return None
    return h155(c)


def h155(c):
    if c is None:
        return None
    return h156(c)


def h156(c):
    if c is None:
        return None
    return h157(c)


def h157(c):
    if c is None:
        return None
    return h158(c)


def h158(c):
    if c is None:
        return None
    return h159(c)


def h159(c):
    if c is None:
        return None
    return h160(c)


def h160(c):
    if c is None:
        return None
    return h161(c)


def h161(c):
    if c is None:
        return None
    return h162(c)


def h162(c):
    if c is None:
        return None
    return h163(c)


def h163(c):
    if c is None:
        return None
    return h164(c)


def h164(c):
    if c is None:
        return None
    return h165(c)


def h165(c):
    if c is None:
        return None
    return h166(c)


def h166(c):
    if c is None:
        return None
    return h167(c)


def h167(c):
    if c is None:
        return None
    return h168(c)


def h168(c):
    if c is None:
        return None
    return h169(c)


def h169(c):
    if c is None:
        return None
    return h170(c)


def h170(c):
    if c is None:
        return None
    return h171(c)


def h171(c):
    if c is None:
        return None
    return h172(c)


def h172(c):
    if c is None:
        return None
    return h173(c)


def h173(c):
    if c is None:
        return None
    return h174(c)


def h174(c):
    if c is None:
        return None
    return h175(c)


def h175(c):
    if c is None:
        return None
    return h176(c)


def h176(c):
    if c is None:
        return None
    return h177(c)


def h177(c):
    if c is None:
        return None
    return h178(c)


def h178(c):
    if c is None:
        return None
    return h179(c)


def h179(c):
    if c is None:
        return None
    return h180(c)


def h180(c):
    if c is None:
        return None
    return h181(c)


def h181(c):
    if c is None:
        return None
    return h182(c)


def h182(c):
    if c is None:
        return None
    return h183(c)


def h183(c):
    if c is None:
        return None
    return h184(c)


def h184(c):
    if c is None:
        return None
    return h185(c)


def h185(c):
    if c is None:
        return None
    return h186(c)


def h186(c):
    if c is None:
        return None
    return h187(c)


def h187(c):
    if c is None:
        return None
    return h188(c)


def h188(c):
    if c is None:
        return None
    return h189(c)


def h189(c):
    if c is None:
        return None
    return h190(c)


def h190(c):
    if c is None:
        return None
    return h191(c)


def h191(c):
    if c is None:
        return None
    return h192(c)


def h192(c):
    if c is None:
        return None
    return h193(c)


def h193(c):
    if c is None:
        return None
    return h194(c)


def h194(c):
    if c is None:
        return None
    return h195(c)


def h195(c):
    if c is None:
        return None
    return h196(c)


def h196(c):
    if c is None:
        return None
    return h197(c)


def h197(c):
    if c is None:
        return None
    return h198(c)


def h198(c):
    if c is None:
        return None
    return h199(c)


def h199(c):
    STATE["n"] = c.__name__
    return c


class Base:
    def __init_subclass__(cls, **kw):
        super().__init_subclass__(**kw)
        REGISTRY[cls.__name__] = cls
        for i in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15):
            for j in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15):
                TABLE[i] = j
        h000(cls)


class C00(Base):
    pass


class C01(Base):
    pass


class C02(Base):
    pass


class C03(Base):
    pass


class C04(Base):
    pass


class C05(Base):
    pass


class C06(Base):
    pass


class C07(Base):
    pass


class C08(Base):
    pass


class C09(Base):
    pass


class C10(Base):
    pass


class C11(Base):
    pass


class C12(Base):
    pass


class C13(Base):
    pass


class C14(Base):
    pass


class C15(Base):
    pass


class C16(Base):
    pass


class C17(Base):
    pass


class C18(Base):
    pass


class C19(Base):
    pass


class C20(Base):
    pass


class C21(Base):
    pass


class C22(Base):
    pass


class C23(Base):
    pass


class C24(Base):
    pass


class C25(Base):
    pass


class C26(Base):
    pass


class C27(Base):
    pass


class C28(Base):
    pass


class C29(Base):
    pass


class C30(Base):
    pass


class C31(Base):
    pass


class C32(Base):
    pass


class C33(Base):
    pass


class C34(Base):
    pass


class C35(Base):
    pass


class C36(Base):
    pass


class C37(Base):
    pass


class C38(Base):
    pass


class C39(Base):
    pass


NAME = "hooklib"
