"""A recursive call answered by its running activation joins the activation's effects (its
footprint) into the caller's state. Here the footprint is 150 records the recursion touches:
joining it entry by entry at every recursive call visited the whole footprint each time; only
where the two states differ (a structural diff: they share most of their structure) can the
join change anything. The work: the walks and the differing entries, with a margin."""
#@visits <= 15000
import sys


def walk(node, seen, depth):
    seen["d%d" % depth] = node
    for child in node.get("children", []):
        walk(child, seen, depth + 1)
    for child in node.get("more", []):
        walk(child, seen, depth + 1)
    return seen


reg0 = {"id": 0}
reg1 = {"id": 1}
reg2 = {"id": 2}
reg3 = {"id": 3}
reg4 = {"id": 4}
reg5 = {"id": 5}
reg6 = {"id": 6}
reg7 = {"id": 7}
reg8 = {"id": 8}
reg9 = {"id": 9}
reg10 = {"id": 10}
reg11 = {"id": 11}
reg12 = {"id": 12}
reg13 = {"id": 13}
reg14 = {"id": 14}
reg15 = {"id": 15}
reg16 = {"id": 16}
reg17 = {"id": 17}
reg18 = {"id": 18}
reg19 = {"id": 19}
reg20 = {"id": 20}
reg21 = {"id": 21}
reg22 = {"id": 22}
reg23 = {"id": 23}
reg24 = {"id": 24}
reg25 = {"id": 25}
reg26 = {"id": 26}
reg27 = {"id": 27}
reg28 = {"id": 28}
reg29 = {"id": 29}
reg30 = {"id": 30}
reg31 = {"id": 31}
reg32 = {"id": 32}
reg33 = {"id": 33}
reg34 = {"id": 34}
reg35 = {"id": 35}
reg36 = {"id": 36}
reg37 = {"id": 37}
reg38 = {"id": 38}
reg39 = {"id": 39}
reg40 = {"id": 40}
reg41 = {"id": 41}
reg42 = {"id": 42}
reg43 = {"id": 43}
reg44 = {"id": 44}
reg45 = {"id": 45}
reg46 = {"id": 46}
reg47 = {"id": 47}
reg48 = {"id": 48}
reg49 = {"id": 49}
reg50 = {"id": 50}
reg51 = {"id": 51}
reg52 = {"id": 52}
reg53 = {"id": 53}
reg54 = {"id": 54}
reg55 = {"id": 55}
reg56 = {"id": 56}
reg57 = {"id": 57}
reg58 = {"id": 58}
reg59 = {"id": 59}
reg60 = {"id": 60}
reg61 = {"id": 61}
reg62 = {"id": 62}
reg63 = {"id": 63}
reg64 = {"id": 64}
reg65 = {"id": 65}
reg66 = {"id": 66}
reg67 = {"id": 67}
reg68 = {"id": 68}
reg69 = {"id": 69}
reg70 = {"id": 70}
reg71 = {"id": 71}
reg72 = {"id": 72}
reg73 = {"id": 73}
reg74 = {"id": 74}
reg75 = {"id": 75}
reg76 = {"id": 76}
reg77 = {"id": 77}
reg78 = {"id": 78}
reg79 = {"id": 79}
reg80 = {"id": 80}
reg81 = {"id": 81}
reg82 = {"id": 82}
reg83 = {"id": 83}
reg84 = {"id": 84}
reg85 = {"id": 85}
reg86 = {"id": 86}
reg87 = {"id": 87}
reg88 = {"id": 88}
reg89 = {"id": 89}
reg90 = {"id": 90}
reg91 = {"id": 91}
reg92 = {"id": 92}
reg93 = {"id": 93}
reg94 = {"id": 94}
reg95 = {"id": 95}
reg96 = {"id": 96}
reg97 = {"id": 97}
reg98 = {"id": 98}
reg99 = {"id": 99}
reg100 = {"id": 100}
reg101 = {"id": 101}
reg102 = {"id": 102}
reg103 = {"id": 103}
reg104 = {"id": 104}
reg105 = {"id": 105}
reg106 = {"id": 106}
reg107 = {"id": 107}
reg108 = {"id": 108}
reg109 = {"id": 109}
reg110 = {"id": 110}
reg111 = {"id": 111}
reg112 = {"id": 112}
reg113 = {"id": 113}
reg114 = {"id": 114}
reg115 = {"id": 115}
reg116 = {"id": 116}
reg117 = {"id": 117}
reg118 = {"id": 118}
reg119 = {"id": 119}
reg120 = {"id": 120}
reg121 = {"id": 121}
reg122 = {"id": 122}
reg123 = {"id": 123}
reg124 = {"id": 124}
reg125 = {"id": 125}
reg126 = {"id": 126}
reg127 = {"id": 127}
reg128 = {"id": 128}
reg129 = {"id": 129}
reg130 = {"id": 130}
reg131 = {"id": 131}
reg132 = {"id": 132}
reg133 = {"id": 133}
reg134 = {"id": 134}
reg135 = {"id": 135}
reg136 = {"id": 136}
reg137 = {"id": 137}
reg138 = {"id": 138}
reg139 = {"id": 139}
reg140 = {"id": 140}
reg141 = {"id": 141}
reg142 = {"id": 142}
reg143 = {"id": 143}
reg144 = {"id": 144}
reg145 = {"id": 145}
reg146 = {"id": 146}
reg147 = {"id": 147}
reg148 = {"id": 148}
reg149 = {"id": 149}


def touch(node):
    reg0["seen"] = node
    reg1["seen"] = node
    reg2["seen"] = node
    reg3["seen"] = node
    reg4["seen"] = node
    reg5["seen"] = node
    reg6["seen"] = node
    reg7["seen"] = node
    reg8["seen"] = node
    reg9["seen"] = node
    reg10["seen"] = node
    reg11["seen"] = node
    reg12["seen"] = node
    reg13["seen"] = node
    reg14["seen"] = node
    reg15["seen"] = node
    reg16["seen"] = node
    reg17["seen"] = node
    reg18["seen"] = node
    reg19["seen"] = node
    reg20["seen"] = node
    reg21["seen"] = node
    reg22["seen"] = node
    reg23["seen"] = node
    reg24["seen"] = node
    reg25["seen"] = node
    reg26["seen"] = node
    reg27["seen"] = node
    reg28["seen"] = node
    reg29["seen"] = node
    reg30["seen"] = node
    reg31["seen"] = node
    reg32["seen"] = node
    reg33["seen"] = node
    reg34["seen"] = node
    reg35["seen"] = node
    reg36["seen"] = node
    reg37["seen"] = node
    reg38["seen"] = node
    reg39["seen"] = node
    reg40["seen"] = node
    reg41["seen"] = node
    reg42["seen"] = node
    reg43["seen"] = node
    reg44["seen"] = node
    reg45["seen"] = node
    reg46["seen"] = node
    reg47["seen"] = node
    reg48["seen"] = node
    reg49["seen"] = node
    reg50["seen"] = node
    reg51["seen"] = node
    reg52["seen"] = node
    reg53["seen"] = node
    reg54["seen"] = node
    reg55["seen"] = node
    reg56["seen"] = node
    reg57["seen"] = node
    reg58["seen"] = node
    reg59["seen"] = node
    reg60["seen"] = node
    reg61["seen"] = node
    reg62["seen"] = node
    reg63["seen"] = node
    reg64["seen"] = node
    reg65["seen"] = node
    reg66["seen"] = node
    reg67["seen"] = node
    reg68["seen"] = node
    reg69["seen"] = node
    reg70["seen"] = node
    reg71["seen"] = node
    reg72["seen"] = node
    reg73["seen"] = node
    reg74["seen"] = node
    reg75["seen"] = node
    reg76["seen"] = node
    reg77["seen"] = node
    reg78["seen"] = node
    reg79["seen"] = node
    reg80["seen"] = node
    reg81["seen"] = node
    reg82["seen"] = node
    reg83["seen"] = node
    reg84["seen"] = node
    reg85["seen"] = node
    reg86["seen"] = node
    reg87["seen"] = node
    reg88["seen"] = node
    reg89["seen"] = node
    reg90["seen"] = node
    reg91["seen"] = node
    reg92["seen"] = node
    reg93["seen"] = node
    reg94["seen"] = node
    reg95["seen"] = node
    reg96["seen"] = node
    reg97["seen"] = node
    reg98["seen"] = node
    reg99["seen"] = node
    reg100["seen"] = node
    reg101["seen"] = node
    reg102["seen"] = node
    reg103["seen"] = node
    reg104["seen"] = node
    reg105["seen"] = node
    reg106["seen"] = node
    reg107["seen"] = node
    reg108["seen"] = node
    reg109["seen"] = node
    reg110["seen"] = node
    reg111["seen"] = node
    reg112["seen"] = node
    reg113["seen"] = node
    reg114["seen"] = node
    reg115["seen"] = node
    reg116["seen"] = node
    reg117["seen"] = node
    reg118["seen"] = node
    reg119["seen"] = node
    reg120["seen"] = node
    reg121["seen"] = node
    reg122["seen"] = node
    reg123["seen"] = node
    reg124["seen"] = node
    reg125["seen"] = node
    reg126["seen"] = node
    reg127["seen"] = node
    reg128["seen"] = node
    reg129["seen"] = node
    reg130["seen"] = node
    reg131["seen"] = node
    reg132["seen"] = node
    reg133["seen"] = node
    reg134["seen"] = node
    reg135["seen"] = node
    reg136["seen"] = node
    reg137["seen"] = node
    reg138["seen"] = node
    reg139["seen"] = node
    reg140["seen"] = node
    reg141["seen"] = node
    reg142["seen"] = node
    reg143["seen"] = node
    reg144["seen"] = node
    reg145["seen"] = node
    reg146["seen"] = node
    reg147["seen"] = node
    reg148["seen"] = node
    reg149["seen"] = node


def walk2(node, depth):
    touch(node)
    for child in node.get("children", []):
        walk2(child, depth + 1)
    for child in node.get("more", []):
        walk2(child, depth + 1)


tree = {"children": [{"children": []}], "more": [{"more": []}]}
walk(tree, {}, 0)
walk2(tree, 0)
import json
t2 = json.loads(sys.argv[1] if len(sys.argv) > 1 else "{}")
walk2(t2, 0)
