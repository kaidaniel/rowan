"""Two hundred and sixty models registered at import, each under its name split off its tag
(`split_name`, as timm's `split_model_name_tag`): the helper's result tuple is made in more
contexts than an allocation site keeps apart."""

REGISTRY = {}


def split_name(name):
    base, *rest = name.split(".", 1)
    return base, (rest[0] if rest else "")


def arch(name):
    return split_name(name)[0]


def register(fn):
    base, tag = split_name(fn.__name__)
    REGISTRY[base] = fn
    return fn


@register
def m000():
    return 0


@register
def m001():
    return 1


@register
def m002():
    return 2


@register
def m003():
    return 3


@register
def m004():
    return 4


@register
def m005():
    return 5


@register
def m006():
    return 6


@register
def m007():
    return 7


@register
def m008():
    return 8


@register
def m009():
    return 9


@register
def m010():
    return 10


@register
def m011():
    return 11


@register
def m012():
    return 12


@register
def m013():
    return 13


@register
def m014():
    return 14


@register
def m015():
    return 15


@register
def m016():
    return 16


@register
def m017():
    return 17


@register
def m018():
    return 18


@register
def m019():
    return 19


@register
def m020():
    return 20


@register
def m021():
    return 21


@register
def m022():
    return 22


@register
def m023():
    return 23


@register
def m024():
    return 24


@register
def m025():
    return 25


@register
def m026():
    return 26


@register
def m027():
    return 27


@register
def m028():
    return 28


@register
def m029():
    return 29


@register
def m030():
    return 30


@register
def m031():
    return 31


@register
def m032():
    return 32


@register
def m033():
    return 33


@register
def m034():
    return 34


@register
def m035():
    return 35


@register
def m036():
    return 36


@register
def m037():
    return 37


@register
def m038():
    return 38


@register
def m039():
    return 39


@register
def m040():
    return 40


@register
def m041():
    return 41


@register
def m042():
    return 42


@register
def m043():
    return 43


@register
def m044():
    return 44


@register
def m045():
    return 45


@register
def m046():
    return 46


@register
def m047():
    return 47


@register
def m048():
    return 48


@register
def m049():
    return 49


@register
def m050():
    return 50


@register
def m051():
    return 51


@register
def m052():
    return 52


@register
def m053():
    return 53


@register
def m054():
    return 54


@register
def m055():
    return 55


@register
def m056():
    return 56


@register
def m057():
    return 57


@register
def m058():
    return 58


@register
def m059():
    return 59


@register
def m060():
    return 60


@register
def m061():
    return 61


@register
def m062():
    return 62


@register
def m063():
    return 63


@register
def m064():
    return 64


@register
def m065():
    return 65


@register
def m066():
    return 66


@register
def m067():
    return 67


@register
def m068():
    return 68


@register
def m069():
    return 69


@register
def m070():
    return 70


@register
def m071():
    return 71


@register
def m072():
    return 72


@register
def m073():
    return 73


@register
def m074():
    return 74


@register
def m075():
    return 75


@register
def m076():
    return 76


@register
def m077():
    return 77


@register
def m078():
    return 78


@register
def m079():
    return 79


@register
def m080():
    return 80


@register
def m081():
    return 81


@register
def m082():
    return 82


@register
def m083():
    return 83


@register
def m084():
    return 84


@register
def m085():
    return 85


@register
def m086():
    return 86


@register
def m087():
    return 87


@register
def m088():
    return 88


@register
def m089():
    return 89


@register
def m090():
    return 90


@register
def m091():
    return 91


@register
def m092():
    return 92


@register
def m093():
    return 93


@register
def m094():
    return 94


@register
def m095():
    return 95


@register
def m096():
    return 96


@register
def m097():
    return 97


@register
def m098():
    return 98


@register
def m099():
    return 99


@register
def m100():
    return 100


@register
def m101():
    return 101


@register
def m102():
    return 102


@register
def m103():
    return 103


@register
def m104():
    return 104


@register
def m105():
    return 105


@register
def m106():
    return 106


@register
def m107():
    return 107


@register
def m108():
    return 108


@register
def m109():
    return 109


@register
def m110():
    return 110


@register
def m111():
    return 111


@register
def m112():
    return 112


@register
def m113():
    return 113


@register
def m114():
    return 114


@register
def m115():
    return 115


@register
def m116():
    return 116


@register
def m117():
    return 117


@register
def m118():
    return 118


@register
def m119():
    return 119


@register
def m120():
    return 120


@register
def m121():
    return 121


@register
def m122():
    return 122


@register
def m123():
    return 123


@register
def m124():
    return 124


@register
def m125():
    return 125


@register
def m126():
    return 126


@register
def m127():
    return 127


@register
def m128():
    return 128


@register
def m129():
    return 129


@register
def m130():
    return 130


@register
def m131():
    return 131


@register
def m132():
    return 132


@register
def m133():
    return 133


@register
def m134():
    return 134


@register
def m135():
    return 135


@register
def m136():
    return 136


@register
def m137():
    return 137


@register
def m138():
    return 138


@register
def m139():
    return 139


@register
def m140():
    return 140


@register
def m141():
    return 141


@register
def m142():
    return 142


@register
def m143():
    return 143


@register
def m144():
    return 144


@register
def m145():
    return 145


@register
def m146():
    return 146


@register
def m147():
    return 147


@register
def m148():
    return 148


@register
def m149():
    return 149


@register
def m150():
    return 150


@register
def m151():
    return 151


@register
def m152():
    return 152


@register
def m153():
    return 153


@register
def m154():
    return 154


@register
def m155():
    return 155


@register
def m156():
    return 156


@register
def m157():
    return 157


@register
def m158():
    return 158


@register
def m159():
    return 159


@register
def m160():
    return 160


@register
def m161():
    return 161


@register
def m162():
    return 162


@register
def m163():
    return 163


@register
def m164():
    return 164


@register
def m165():
    return 165


@register
def m166():
    return 166


@register
def m167():
    return 167


@register
def m168():
    return 168


@register
def m169():
    return 169


@register
def m170():
    return 170


@register
def m171():
    return 171


@register
def m172():
    return 172


@register
def m173():
    return 173


@register
def m174():
    return 174


@register
def m175():
    return 175


@register
def m176():
    return 176


@register
def m177():
    return 177


@register
def m178():
    return 178


@register
def m179():
    return 179


@register
def m180():
    return 180


@register
def m181():
    return 181


@register
def m182():
    return 182


@register
def m183():
    return 183


@register
def m184():
    return 184


@register
def m185():
    return 185


@register
def m186():
    return 186


@register
def m187():
    return 187


@register
def m188():
    return 188


@register
def m189():
    return 189


@register
def m190():
    return 190


@register
def m191():
    return 191


@register
def m192():
    return 192


@register
def m193():
    return 193


@register
def m194():
    return 194


@register
def m195():
    return 195


@register
def m196():
    return 196


@register
def m197():
    return 197


@register
def m198():
    return 198


@register
def m199():
    return 199


@register
def m200():
    return 200


@register
def m201():
    return 201


@register
def m202():
    return 202


@register
def m203():
    return 203


@register
def m204():
    return 204


@register
def m205():
    return 205


@register
def m206():
    return 206


@register
def m207():
    return 207


@register
def m208():
    return 208


@register
def m209():
    return 209


@register
def m210():
    return 210


@register
def m211():
    return 211


@register
def m212():
    return 212


@register
def m213():
    return 213


@register
def m214():
    return 214


@register
def m215():
    return 215


@register
def m216():
    return 216


@register
def m217():
    return 217


@register
def m218():
    return 218


@register
def m219():
    return 219


@register
def m220():
    return 220


@register
def m221():
    return 221


@register
def m222():
    return 222


@register
def m223():
    return 223


@register
def m224():
    return 224


@register
def m225():
    return 225


@register
def m226():
    return 226


@register
def m227():
    return 227


@register
def m228():
    return 228


@register
def m229():
    return 229


@register
def m230():
    return 230


@register
def m231():
    return 231


@register
def m232():
    return 232


@register
def m233():
    return 233


@register
def m234():
    return 234


@register
def m235():
    return 235


@register
def m236():
    return 236


@register
def m237():
    return 237


@register
def m238():
    return 238


@register
def m239():
    return 239


@register
def m240():
    return 240


@register
def m241():
    return 241


@register
def m242():
    return 242


@register
def m243():
    return 243


@register
def m244():
    return 244


@register
def m245():
    return 245


@register
def m246():
    return 246


@register
def m247():
    return 247


@register
def m248():
    return 248


@register
def m249():
    return 249


@register
def m250():
    return 250


@register
def m251():
    return 251


@register
def m252():
    return 252


@register
def m253():
    return 253


@register
def m254():
    return 254


@register
def m255():
    return 255


@register
def m256():
    return 256


@register
def m257():
    return 257


@register
def m258():
    return 258


@register
def m259():
    return 259
