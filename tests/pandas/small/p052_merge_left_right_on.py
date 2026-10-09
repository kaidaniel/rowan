# merge with left_on / right_on: both key columns stay; a left column named like the right key gets suffixes
import pandas as pd

ORDERS = pd.DataFrame({"order": [1, 2, 3], "cust_id": [10, 11, 10], "amount": [5.0, 6.0, 7.0]})
CUST = pd.DataFrame({"id": [10, 11], "name": ["a", "b"]})
CUST_T = pd.DataFrame({"id": [10], "name": ["a"], "tier": ["gold"]})


def link(o, c):
    m = o.merge(c, left_on="cust_id", right_on="id", how="left")   #@ ok cust_id ; ok id
    #@row m: present={order,cust_id,amount,id,name,tier} definite={order,cust_id,amount,id,name} closed
    m["id"]                                              #@ ok id
    m["cust_id"]                                         #@ ok cust_id
    return m


def tier(m):
    return m["tier"]                                     #@ possibly tier


def clash(o, c):
    o2 = o.rename(columns={"order": "id"})
    mm = o2.merge(c, left_on="cust_id", right_on="id")
    #@row mm: present={id_x,cust_id,amount,id_y,name,tier} definite={id_x,cust_id,amount,id_y,name} absent={id,order} closed
    return mm["id"]                                      #@ missing id


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    link(ORDERS, CUST)
    tier(link(ORDERS, CUST_T))
    try:
        tier(link(ORDERS, CUST))
    except KeyError as e:
        kept.append(e)
    for c in (CUST, CUST_T):
        try:
            clash(ORDERS, c)
        except KeyError as e:
            kept.append(e)
