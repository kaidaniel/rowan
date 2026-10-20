# The keys of a dict with more keys than rowan names one by one (70 here), collected into a
# list while the dict has them, are the keys it had then: keys it loses later are still the
# list's (`seen` is filled under them: not "never set"; under keys not known, so no verdict), keys it gains later are not (a loop
# storing under renamed keys converges at once instead of renaming its own new keys again).
D = {"k_0": 0, "k_1": 1, "k_2": 2, "k_3": 3, "k_4": 4, "k_5": 5, "k_6": 6, "k_7": 7, "k_8": 8, "k_9": 9, "k_10": 10, "k_11": 11, "k_12": 12, "k_13": 13, "k_14": 14, "k_15": 15, "k_16": 16, "k_17": 17, "k_18": 18, "k_19": 19, "k_20": 20, "k_21": 21, "k_22": 22, "k_23": 23, "k_24": 24, "k_25": 25, "k_26": 26, "k_27": 27, "k_28": 28, "k_29": 29, "k_30": 30, "k_31": 31, "k_32": 32, "k_33": 33, "k_34": 34, "k_35": 35, "k_36": 36, "k_37": 37, "k_38": 38, "k_39": 39, "k_40": 40, "k_41": 41, "k_42": 42, "k_43": 43, "k_44": 44, "k_45": 45, "k_46": 46, "k_47": 47, "k_48": 48, "k_49": 49, "k_50": 50, "k_51": 51, "k_52": 52, "k_53": 53, "k_54": 54, "k_55": 55, "k_56": 56, "k_57": 57, "k_58": 58, "k_59": 59, "k_60": 60, "k_61": 61, "k_62": 62, "k_63": 63, "k_64": 64, "k_65": 65, "k_66": 66, "k_67": 67, "k_68": 68, "k_69": 69}
names = [n for n in D]
D.clear()
seen = {}
for n in names:
    seen[n] = 1
x = seen["k_5"]  #@ sound k_5
#@value n: types={str}

R = {"m_0_a": 0, "m_1_a": 1, "m_2_a": 2, "m_3_a": 3, "m_4_a": 4, "m_5_a": 5, "m_6_a": 6, "m_7_a": 7, "m_8_a": 8, "m_9_a": 9, "m_10_a": 10, "m_11_a": 11, "m_12_a": 12, "m_13_a": 13, "m_14_a": 14, "m_15_a": 15, "m_16_a": 16, "m_17_a": 17, "m_18_a": 18, "m_19_a": 19, "m_20_a": 20, "m_21_a": 21, "m_22_a": 22, "m_23_a": 23, "m_24_a": 24, "m_25_a": 25, "m_26_a": 26, "m_27_a": 27, "m_28_a": 28, "m_29_a": 29, "m_30_a": 30, "m_31_a": 31, "m_32_a": 32, "m_33_a": 33, "m_34_a": 34, "m_35_a": 35, "m_36_a": 36, "m_37_a": 37, "m_38_a": 38, "m_39_a": 39, "m_40_a": 40, "m_41_a": 41, "m_42_a": 42, "m_43_a": 43, "m_44_a": 44, "m_45_a": 45, "m_46_a": 46, "m_47_a": 47, "m_48_a": 48, "m_49_a": 49, "m_50_a": 50, "m_51_a": 51, "m_52_a": 52, "m_53_a": 53, "m_54_a": 54, "m_55_a": 55, "m_56_a": 56, "m_57_a": 57, "m_58_a": 58, "m_59_a": 59, "m_60_a": 60, "m_61_a": 61, "m_62_a": 62, "m_63_a": 63, "m_64_a": 64, "m_65_a": 65, "m_66_a": 66, "m_67_a": 67, "m_68_a": 68, "m_69_a": 69}
todo = [k for k in R if R[k] > 3]
for t in todo:
    R[t.replace("_a", "_a_b")] = 1
y = R["m_1_a"]  #@ proven m_1_a
#@value t: types={str}
#@work <= 30
