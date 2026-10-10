# Reporting tool over a package: configparser on a literal INI + os.environ overrides set by the driver, csv.DictReader / json.loads of literal texts, pandas groupby / agg / merge / sort_values / to_dict, Template / textwrap / datetime formatting, argparse with explicit argv lists
import os

from reporting.cli import build_parser, run
from reporting.config import INI_WITH_FOOTER, apply_env, load_config
from reporting.fmt import money, period_label, summary_line, wrap_note
from reporting.loader import NOTES_CSV, load_products, load_sales
from reporting.transform import by_category, by_region, sales_frame

if __name__ == "__main__":  # todos at L43-L80: exact per-row pandas values (groupby + sort order, filtered row counts) need a concrete table model; frames keep per-column value sets
    os.environ.pop("REPORT_TITLE", None)
    os.environ.pop("REPORT_WIDTH", None)

    # ---------------------------------------------------------------- configuration
    cfg = load_config()
    #@row cfg: present={title,currency,min_qty,regions,width,format} definite={title,currency,min_qty,regions,width,format} absent={footer} closed
    mq = cfg["min_qty"]                              #@value mq: exactly={1}
    cur = cfg["currency"]                            #@value cur: exactly={"EUR"}
    cfg_f = load_config(INI_WITH_FOOTER)
    #@row cfg_f: present={title,currency,min_qty,regions,width,format,footer} definite={title,currency,min_qty,regions,width,format,footer} closed
    nreg = len(cfg_f["regions"])                     #@value nreg: exactly={3}
    plain = apply_env(cfg)
    #@row plain: present={title,currency,min_qty,regions,width,format,overridden} definite={title,currency,min_qty,regions,width,format,overridden} closed
    nover = len(plain["overridden"])                 #@value nover: exactly={0}

    # ---------------------------------------------------------------- loading
    sales = load_sales(min_qty=1)
    ns = len(sales)                                  #@value ns: exactly={5}
    s0 = sales[0]                                    #@row s0: present={date,region,rep,sku,qty,price} definite={date,region,rep,sku,qty,price} absent={note} closed
    notes = load_sales(NOTES_CSV)
    n0 = notes[0]                                    #@row n0: present={date,region,rep,sku,qty,price,note} definite={date,region,rep,sku,qty,price,note} closed
    n1 = notes[1]                                    #@row n1: present={date,region,rep,sku,qty,price} definite={date,region,rep,sku,qty,price} absent={note} closed
    #@schema param parse_sale.row: always={date,region,rep,sku,qty,price} only={date,region,rep,sku,qty,price}
    products = load_products()                       #@row products: present={A1,B2,C3} definite={A1,B2,C3} closed
    c3cat = products["C3"]["category"]               #@value c3cat: exactly={"misc"}
    #@schema param product_entry.p: always={sku,name} only={sku,name}

    # ---------------------------------------------------------------- frames
    df = sales_frame(sales, products)
    #@row df: present={date,region,rep,sku,qty,price,name,category,revenue} definite={date,region,rep,sku,qty,price,name,category,revenue} closed
    reg = by_region(df)                              #@row reg: present={region,orders,qty,revenue} definite={region,orders,qty,revenue} closed
    top_region = reg.to_dict("records")[0]           #@row top_region: present={region,orders,qty,revenue} definite={region,orders,qty,revenue} closed
    tr_name = top_region["region"]                   #@todo value tr_name: exactly={"north"}
    line = summary_line(top_region)                  #@todo value line: exactly={"north: 3 orders / 10 units"}
    cats = by_category(df)                           #@row cats: present={category,qty,revenue} definite={category,qty,revenue} closed
    best_cat = cats.to_dict("records")[0]["category"]   #@todo value best_cat: exactly={"electronics"}

    # ---------------------------------------------------------------- formatting
    period = period_label(["2024-03-09", "2024-03-01"])   #@value period: exactly={"01 Mar to 09 Mar 2024"}
    amount = money(1234.5, "EUR").replace(",", "_")  #@value amount: exactly={"1_234.50 EUR"}
    wrapped = len(wrap_note("rush order for the west region team", 12))   #@value wrapped: exactly={3}

    # ---------------------------------------------------------------- command line
    args = build_parser().parse_args(["regions", "--region", "north", "--region", "east", "--min-qty", "2"])
    #@row args: present={command,regions,top,min_qty,json} definite={command,regions,top,min_qty,json} closed
    top_n = args.top                                 #@value top_n: exactly={2}

    res1 = run(["regions"])
    #@row res1: present={command,header,rows,text,rule} definite={command,header,rows,text,rule} absent={footer} closed
    h1 = res1["header"]                              #@value h1: exactly={"Monthly sales (EUR) - 01 Mar to 09 Mar 2024"}
    nrows1 = len(res1["rows"])                       #@todo value nrows1: exactly={2}
    row1 = res1["rows"][1]                           #@row row1: present={region,orders,qty,revenue} definite={region,orders,qty,revenue} closed

    os.environ["REPORT_TITLE"] = "Q1 sales"
    os.environ["REPORT_WIDTH"] = "30"
    res2 = run(["categories", "--json"], ini=INI_WITH_FOOTER)
    #@row res2: present={command,header,rows,text,rule,footer} definite={command,header,rows,text,rule,footer} closed
    h2 = res2["header"]                              #@value h2: exactly={"Q1 sales (EUR) - 01 Mar to 09 Mar 2024"}
    rule2 = len(res2["rule"])                        #@value rule2: exactly={30}

    os.environ.pop("REPORT_WIDTH")
    res3 = run(["reps", "--top", "1", "--min-qty", "0"])
    rep0 = res3["rows"][0]                           #@row rep0: present={rep,qty,revenue} definite={rep,qty,revenue} closed
    rep_name = rep0["rep"]                           #@todo value rep_name: exactly={"ann"}
    rule3 = len(res3["rule"])                        #@value rule3: exactly={40}

    os.environ.pop("REPORT_TITLE")
    res4 = run(["regions", "--region", "west"], csv_text=NOTES_CSV)
    h4 = res4["header"]                              #@value h4: exactly={"Monthly sales (EUR) - 02 Apr to 03 Apr 2024"}
    west = res4["rows"][0]["revenue"]                #@todo value west: exactly={51.0}
    try:
        run(["totals"])
        bad = False
    except SystemExit:
        bad = True
    _ = 0                                            #@value bad: exactly={True}
