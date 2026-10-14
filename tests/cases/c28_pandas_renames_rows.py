import pandas as pd

df = pd.read_csv("in.csv")
df["Amount Due"]
df1 = df.rename(columns=lambda c: c.strip().lower().replace(" ", "_"))
df1["amount_due"]                                   #@ ok amount_due
df1["Amount Due"]                                   #@ missing "Amount Due"

df2 = df.copy()
df2.columns = [c.upper() for c in df2.columns]
df2["AMOUNT DUE"]                                   #@ ok "AMOUNT DUE"
df2["Amount Due"]                                   #@ missing "Amount Due"

df3 = df.copy()
df3.columns = df3.columns.str.lower()
df3["amount due"]                                   #@ ok "amount due"

for _, row in df.iterrows():
    row["Amount Due"]                               #@ ok "Amount Due"
    row["Customer"]                                 #@ input Customer

recs = df.to_dict("records")
# (was `ok Customer`, from the loop above narrowing the frame; but a frame without rows never
# runs the loop: a row access proves nothing of the frame. Here recs[0] exists only when the
# loop ran, which rowan does not relate)
recs[0]["Customer"]                                 #@ input Customer
#@todo ok Customer
recs[0]["Missing"]                                  #@ input Missing

frame = pd.DataFrame([{"a": 1, "b": 2}, {"a": 3}])
frame["b"]                                          #@ ok b
frame["c"]                                          #@ missing c
sub = frame[["a"]]
sub["b"]                                            #@ missing b
#@schema read_csv@L3: always={"Amount Due",Customer,Missing}
