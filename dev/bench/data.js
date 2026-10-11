window.BENCHMARK_DATA = {
  "lastUpdate": 1791720201256,
  "repoUrl": "https://github.com/kaidaniel/rowan",
  "entries": {
    "rowan": [
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "00922ed4eb4b4551e0366ea2d0e852a5253c968c",
          "message": "Bench history: on main, github-action-benchmark appends each run's numbers (per program: wall time, peak memory, work, units, tabulations, objects) to gh-pages (dev/bench), which is deployed to GitHub Pages (source: GitHub Actions); every commit on main gets its own run; bench/history.py converts the results\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-10T22:12:24+01:00",
          "tree_id": "1ffaea4ab5d45039cd431f761e519d56cbff3dc6",
          "url": "https://github.com/kaidaniel/rowan/commit/00922ed4eb4b4551e0366ea2d0e852a5253c968c"
        },
        "date": 1791667210351,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 300.87,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 546,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 300.82,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 2992,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 34.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1376,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 715322,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3189304,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 41964,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 161884,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "cb9aa9828c850bff255cb8b28de9836dcd1f733e",
          "message": "Two more bounds for large programs: an attribute name of an unknown value that more than 8 user classes define (`render`, `get_style`) is unknown code instead of a call of every class's method (`duck`); a liveness walk for an unknown call's result visits at most 2,000 objects (`livewalkunk`; its records are open anyway). rich without deadlines: memory no longer the 7-15 GB of before at 5 min, still not finished (volume of nested summaries in rich.pretty). Golden 1019/1019, snapshots same\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-10T22:34:34+01:00",
          "tree_id": "d55dc16111385ae03a8ce3f4a793f3e17248aeb7",
          "url": "https://github.com/kaidaniel/rowan/commit/cb9aa9828c850bff255cb8b28de9836dcd1f733e"
        },
        "date": 1791668801409,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 300.97,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1383,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.72,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 3361,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 46.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1369,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 684040,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3441839,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 40909,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 157672,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "02462f5b42cdb338403f7e2be28698ee386d809c",
          "message": "More quadratic spots on large value sets (rich without deadlines): instantiation keeps the set of its clones (was a scan of all clones per new clone and per element target); symbolic requirements' target groups are found by the row's hash; an item read joins all its targets' values at once. Identical results on email / rich / stdlib12k. Golden 1024/1024, snapshots same\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T01:48:22+01:00",
          "tree_id": "d40ab22a643e81b1ed7acdedafd5fe34fb61a6c8",
          "url": "https://github.com/kaidaniel/rowan/commit/02462f5b42cdb338403f7e2be28698ee386d809c"
        },
        "date": 1791703796109,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 301.67,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1558,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.82,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 12729,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 46.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 688892,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3483606,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 41664,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 154320,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "59e3b2f7547dfb9c11a78bb53ac2cd87cbdc6b67",
          "message": "Summary variants removed again (revert of 96fa750): measured at equal work with variants always on and unchecked, re-analyses fall by 0-7 % (with literal outcomes), time is unchanged, and proven accesses drop (email 118 -> 21, stdlib12k 1478 -> 1077). The variants covered only truth / narrow outcomes of fixed-truth objects, a small share of what triggers re-analyses. Golden 1024/1024, snapshots same, cargo test ok\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T08:44:49+01:00",
          "tree_id": "3a1b948b3bac22763e5e064d5aff66ab3af1e349",
          "url": "https://github.com/kaidaniel/rowan/commit/59e3b2f7547dfb9c11a78bb53ac2cd87cbdc6b67"
        },
        "date": 1791707915932,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 301.82,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1490,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.87,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 12732,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 48.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 688892,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3483606,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 41664,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 154320,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "19003d40ad66bb5c149a0028f9b0465ed651452a",
          "message": "Classes deriving from an external base through user classes (email's `Atom(TokenList(list))`) are open like direct subclasses, and instances of such classes have open item rows: `a.append(...)` and `a[0]` on them were \"definitely missing\" (email: 297 such verdicts once the hasattr fix let rowan reach that code; now none). email proven 118 -> 318 at work=550000, stdlib12k / rich unchanged. y448 (new; the oracle agrees). Golden 1026/1026, snapshots same, oracle 864/864, hover test passes. ROWAN_NO=openbaseinherit / openbaseitems: off\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T09:40:32+01:00",
          "tree_id": "5b87f2d6d80e6b2ff60395a494cee8eac144be9b",
          "url": "https://github.com/kaidaniel/rowan/commit/19003d40ad66bb5c149a0028f9b0465ed651452a"
        },
        "date": 1791708699081,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 301.82,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1510,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.72,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 12998,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 33.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 688892,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3483606,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 41664,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 154320,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "6b0d36710e1f074307f637d8f4aa89df0e72deb1",
          "message": "hasattr on a str value is decided by str's attributes (CPython 3.14 `dir(str)` without dunders): a name str does not have is False instead of unknown, and `decode` (bytes', not str's) is no longer True. y449 (new; the oracle agrees). email / stdlib12k / rich unchanged. Golden 1027/1027, snapshots same, oracle 865/865, hover test passes\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T09:58:15+01:00",
          "tree_id": "1333b154b79e9ce668ae78ada68b27cb8fb445dd",
          "url": "https://github.com/kaidaniel/rowan/commit/6b0d36710e1f074307f637d8f4aa89df0e72deb1"
        },
        "date": 1791709562232,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 300.77,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1601,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.77,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 12744,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 50.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 688892,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 3483606,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 41664,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 154320,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "b82fa0981453eb1746f1a6e7fbaa40f10ecc9373",
          "message": "Duck typing stops at the parts of a value duck-typed as several classes: an unknown value or class-less placeholder dispatched to two or more user classes marks its parts (attributes, items, elements at any depth, results of unknown calls on them, a summary parameter bound to one via a signature entry) as not duck-typed in turn, the bound methods of a many-object receiver take only the objects the rule leaves, and a recursive call with such an argument inside the function's own summary run is unknown code (the partial summary duck-types its parameter). Every part of every part was read as every class up to the access-path depth: rich's pretty printer made 1.3M objects. rich at work=550000: objects 1.33M -> 291k, 110 s -> 15 s, 5.2 GB -> 1.6 GB (palette.py:35 loses a proven run, table.py:442 gains one); email / stdlib12k unchanged but one verdict. y446 (new: a pretty printer over unknown values, 8152 -> under 2000 objects; the oracle agrees) with the new golden directive `#@objects <= N` (golden JSON carries the object count). Snapshots reviewed: proto_summary loses 6 \"sometimes\" schema entries on fields of duck-typed results, the rest are object renamings. Golden 1028/1028, oracle 866/866, cargo test, hover test pass. ROWAN_NO=duckparts / duckrec: off\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T10:04:05+01:00",
          "tree_id": "f5155c40c5c6bf5a52c72324d64895dc1fa184d8",
          "url": "https://github.com/kaidaniel/rowan/commit/b82fa0981453eb1746f1a6e7fbaa40f10ecc9373"
        },
        "date": 1791709899001,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 301.72,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1367,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 301.87,
            "unit": "s",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 7795,
            "unit": "MB",
            "extra": "cut at the 300 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 24.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 975,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 575981,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2638661,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36506,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 135638,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "7f56b7b3c9f54702d7db2d2b9897b3184264b78a",
          "message": "Bench: 30 minute timeout per program (was 5), job limit 60 minutes\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T10:19:32+01:00",
          "tree_id": "e2cc8d9c47fe5fb487e2f2ab4b564cb60d4ec1ab",
          "url": "https://github.com/kaidaniel/rowan/commit/7f56b7b3c9f54702d7db2d2b9897b3184264b78a"
        },
        "date": 1791712374188,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.37,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 3184,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1806.48,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 13985,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 25.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 975,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 575981,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2638661,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36506,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 135638,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "6a14143be925c804258e67914aaa2c06981f2142",
          "message": "An unknown call's result object is keyed by the last call site of its context only (k = 1) instead of the whole frame context (call strings of up to three call sites, receiver contexts): every context made its own result and its own tree of parts (rich's pretty printer over unknown values). ROWAN_LIMITS=unkretk=0: no context, =2 (any value >= 2): the whole context, the old behaviour.\n\nMeasured at work=550000 (whole context / k = 1 / k = 0):\n  rich       objects 291k / 146k / 183k; 13.2 / 9.4 / 10.4 s; 1.6 / 1.0 / 1.1 GB; proven 603 / 603 / 603, verdicts identical\n  stdlib12k  objects 51.0k / 43.5k / 42.8k; proven 1478 / 1220 / 1084; missing 788 / 577 / 619; input 3643 / 3574 / 3219; possibly 4313 / 4347 / 4225\n  email      objects 38.4k / 37.5k / 37.3k; proven 317 / 317 / 317; input 472 / 490 / 487\n  standalone copy of rich's pretty.py (554 lines, entry runs): objects 21.0k / 16.4k / 8.5k\nstdlib12k loses 17 % of its proven accesses at k = 1 (not investigated). Golden 1028/1028 at every k (no case depends on this context), snapshots: objects renamed only (proto_domain 27, proto_summary 40 records; same verdicts, keys, tiers and lines), oracle 866/866, cargo test and hover test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T10:20:08+01:00",
          "tree_id": "a92bfc7f703eb0c3e3909715459676436c9127f5",
          "url": "https://github.com/kaidaniel/rowan/commit/6a14143be925c804258e67914aaa2c06981f2142"
        },
        "date": 1791712401491,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.43,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 2310,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1807.19,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 13891,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 30.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 961,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 567946,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2480950,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 37120,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 126077,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "de1bbe65befeb6d860adc1da91ab2f322b4057ce",
          "message": "AGENTS.md: references to SUGGESTED_CHANGES items that were removed (genfull's measurement is in 1ba1df6), the known problems and next steps as they stand\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T10:22:41+01:00",
          "tree_id": "35b11d7c45a99ee5e99eb545483f87cf82804d67",
          "url": "https://github.com/kaidaniel/rowan/commit/de1bbe65befeb6d860adc1da91ab2f322b4057ce"
        },
        "date": 1791712554618,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.48,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 2306,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1806.38,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 13976,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 32.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 962,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 567946,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2480950,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 37120,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 126077,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "65d65d3e70892d78abc3c2d04f91707d849c4931",
          "message": "The tabulation budget belongs to a function's code (`FuncNode::addr`), not to each function object: a `def` inside a generator or another tabulated function is a new closure object at every run of its parent, and each object had a 256 budget of its own (email without deadlines: `FeedParser._parsegen.boundarymatch` as 12 objects, 3,072 tabulations and counting). Every tabulated body run is counted (`tab_runs`, forced ones included): the `work:` line reports it as `tabulations` (the budgeted ones after `budgeted tabulations`), golden JSON as `tabulations`.\n\nGolden directives `#@tabulations <= N` (a bound on the per-call-site body runs) and `#@limits k=v,…` (the case's ROWAN_LIMITS). y450 (new, expectations by hand: a closure made by a generator, tab=4: at most 20 tabulations, 37+ before; the oracle agrees).\n\nBench, work=550000: email / stdlib12k / rich results unchanged (email 9,855 tabulations of which 2,927 budgeted: the forced ones are next). Snapshots reviewed: proto_summary loses \"sometimes\" schema entries on results of a closure's calls (`SummaryMixin()['alloc']`) and gains verdicts by other call chains, as its closures now share one budget. Golden 1029/1029, cargo test, snapshots, hover test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T11:18:55+01:00",
          "tree_id": "d0ef92427702eb3e384336ffe5f992bcce391dd2",
          "url": "https://github.com/kaidaniel/rowan/commit/65d65d3e70892d78abc3c2d04f91707d849c4931"
        },
        "date": 1791715897126,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.28,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 2137,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1807.58,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 14026,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 35.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 946,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 560904,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2416484,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 37452,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 123883,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "19569c23feb6f567d7b2d0935c8bce44862ce0d5",
          "message": "Generators: a `while True:` is unrolled only outside other loops and unrolled iterations (nested unrolling multiplied the iterations by 16 per level: email's `FeedParser._parsegen` nests three and held 4,097 yield states per run), and the join a caller sees over a generator's yield states diffs each yield state against the one before it and joins each distinct row once (it joined every touched fact with every yield state: 77M row joins on email). Yield states are kept with an index by key (`YieldStates`). The tabulation counter counts body runs (recursive calls answered by a running activation return without one).\n\nGolden: JSON carries `joins` (joins of two different rows, a deterministic cost counter, also on the `work:` line) and `work`; directives `#@joins <= N` and `#@work <= N`. y451 (new: three nested `while True:` in a generator, 10,212 objects before, under 600 now) and y452 (new: a generator toggling 12 records between 384 yields, 19,584 row joins before, under 12,000 now); expectations by hand, the oracle agrees. `ROWAN_DEBUG=1` prints `tabrun F (why)` for every per-call-site run.\n\nBench, work=550000: email 31 s -> 6.8 s (unloaded machine), reaching further: proven 317 -> 355, schema entries 99 -> 136, row joins 5.0M (without the join change) -> 0.78M; one \"likely-missing\" lost (main.py `msg[\"Subject\"]`). Its new `missing` verdicts on `StringIO` methods (feedparser.py:104-113) and `HeaderRegistry.__getitem__` predate this change (the previous build gives them with a larger budget): next. stdlib12k / rich results unchanged. Golden 1031/1031, oracle 869/869, cargo test, snapshots (unchanged), hover test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T11:55:47+01:00",
          "tree_id": "5e5e612ebfb5309289d1b881ac4a45f57ccff15d",
          "url": "https://github.com/kaidaniel/rowan/commit/19569c23feb6f567d7b2d0935c8bce44862ce0d5"
        },
        "date": 1791718154879,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.38,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1694,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1806.48,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 13979,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 23.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 948,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 560904,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2416484,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 28301,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 123883,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "ad57ae45b42495e11e7223e22a4a6cb08d33377f",
          "message": "Soundness: a summary's attribute and item reads replayed on the objects a call passes are no longer classified against rows that do not decide them: a class object has the dunders of `type` and `object` and `mro` (and what its MRO defines); an object a model simulates (StringIO, compiled pattern, pydantic error, FastAPI objects) has the model's attributes, its row holds the model's own state; an instance whose class defines `__getattr__` / `__getattribute__` answers any attribute; a subscript of an instance whose class defines `__getitem__` calls it. Each was reported \"definitely missing\" (email: `cls.__name__` in `HeaderRegistry.__getitem__`, `StringIO.write` / `seek` / `readlines` / `truncate` in `BufferedSubFile.push`, `self[name]` in `HeaderRegistry.__call__`, met once the budgets were spent).\n\ny453-y455 (new, expectations by hand; the oracle agrees; each failed before). ROWAN_NO=classdunders / modelattrs / getattrcls / getitemcls: off.\n\nBench, work=550000: email missing 6 -> 0; stdlib12k missing 577 -> 567; rich objects 146k -> 44k, 981 -> 440 MB, verdicts unchanged. Golden 1034/1034, oracle 872/872, cargo test, snapshots (unchanged) pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T12:18:53+01:00",
          "tree_id": "10275537835f7f3db735efae3be0588362e3be8f",
          "url": "https://github.com/kaidaniel/rowan/commit/ad57ae45b42495e11e7223e22a4a6cb08d33377f"
        },
        "date": 1791719533270,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.42,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1672,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.38,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 7353,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 32.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 947,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 560904,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 2416464,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 28301,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 123839,
            "unit": "count",
            "extra": "finished"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "committer": {
            "email": "kaiingodaniel@gmail.com",
            "name": "Kai Daniel",
            "username": "kaidaniel"
          },
          "distinct": true,
          "id": "44edd3a7258808d71bd572eea4d5a23be8fb0421",
          "message": "Soundness: a class's own `__new__` is called: `Cls(args)` is what `__new__(Cls, args)` returns, with `__init__` run on it when it is an instance of `Cls`; `object.__new__(cls)` (also `super().__new__(cls)`, a static method: not bound) is a fresh instance of each user class `cls` may be. `__new__` was ignored: the instance was a fresh empty one, and what `__new__` set read as definitely missing (`_pydecimal.Decimal` builds itself in `__new__`). Every instance has `object`'s attributes and its class's `__module__` / `__doc__` / `__dict__` / `__weakref__` (`OBJECT_ATTRS`), at module level and replayed from summaries (they were definitely missing; `__class__` / `__dict__` were already handled live).\n\ny456, y457 (new, expectations by hand; the oracle agrees; each failed before). ROWAN_NO=usernew / objectattrs: off.\n\nBench, work=550000: stdlib12k missing 567 -> 153 (all 412 were `Decimal` attributes set in `__new__` and instance dunders). Golden 1036/1036, oracle 874/874, cargo test, snapshots (unchanged) pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T12:29:53+01:00",
          "tree_id": "4108117c23705c20c8fa2c0246ee4a8e64d37865",
          "url": "https://github.com/kaidaniel/rowan/commit/44edd3a7258808d71bd572eea4d5a23be8fb0421"
        },
        "date": 1791720200305,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.22,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.27,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 9940,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 139.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2239,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1203429,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 21233474,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 34329,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 180171,
            "unit": "count",
            "extra": "finished"
          }
        ]
      }
    ]
  }
}