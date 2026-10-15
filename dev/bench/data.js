window.BENCHMARK_DATA = {
  "lastUpdate": 1792025490844,
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
          "id": "54ae9089b0bb35933e42af8713cce8f6fe33a07b",
          "message": "Soundness: `self.x = v` in a method's summary, on a placeholder of a class with `__setattr__`, calls it (the hook was looked up on instances only: the store was dropped, and what `__init__` set through `__setattr__` read as definitely missing: `_pydecimal.Context.prec`); a placeholder of a user class tested by `isinstance` against builtin classes only is not one of them when no subclass of its class in the program has an external base (the branch's accesses were replayed on its instances: `value.strip` on a `Decimal`); a class-less placeholder tested against a single builtin class is narrowed to it (its branch's accesses replay only on its instances).\n\ny458, y459 (new, expectations by hand; the oracle agrees; each failed before). ROWAN_NO=phmethod / phbuiltin / narrowbuiltin: off.\n\nBench, work=550000: stdlib12k missing 153 -> 17, proven 1422 -> 1601; email / rich unchanged. Snapshots reviewed: proto_domain / proto_summary verdicts renamed (same tiers, no new `missing`). Golden 1038/1038, oracle 876/876, cargo test, hover test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T12:46:50+01:00",
          "tree_id": "2e2f7213f361592b39a18d0d9b19f38847343faf",
          "url": "https://github.com/kaidaniel/rowan/commit/54ae9089b0bb35933e42af8713cce8f6fe33a07b"
        },
        "date": 1791721186790,
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
            "value": 1625,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.42,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 8444,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 167.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2063,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1218064,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 22624872,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 34477,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 179890,
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
          "id": "724672b59eb241e3b39752ffdb6f45ec41160886",
          "message": "Soundness: `*args` in a call of a dataclass / NamedTuple class supplies positional fields (a list / tuple of known length its positions in order; one of unknown length its element to each further positional field, which is then definitely there when the call returns), and a function / class / lambda defined in an `except` handler has its own scope (the scope pass walked a copy of each handler: scopes are keyed by node address, so such a function took the module's scope and every module name it read was an unbound local). Both were reported definitely missing (rich: `ColorTriplet(*background)`'s fields, `WindowsConsoleFeatures` in `get_windows_console_features`).\n\ny460, y461 (new, expectations by hand; the oracle agrees; each failed before). ROWAN_NO=dcstarargs: off.\n\nBench, work=550000: rich missing 36 -> 0 (see the bench line in the round's notes); email / stdlib12k unchanged. Golden 1040/1040, oracle 878/878, cargo test, snapshots (unchanged) pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T12:56:39+01:00",
          "tree_id": "7b4f6ecf5dcbd311061cebb385b74f39c69eca8d",
          "url": "https://github.com/kaidaniel/rowan/commit/724672b59eb241e3b39752ffdb6f45ec41160886"
        },
        "date": 1791721776031,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1804.22,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1700,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.22,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 9960,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 100.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2057,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1218061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 22624869,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 34477,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 179884,
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
          "id": "587fad44c62c19018a4b524cfab7cf1d75c07cb9",
          "message": "Performance: an unknown call's result allocated again at its site is taken as still in use without a liveness walk, and merged (its rows are open anyway): rich's pretty printer made 350k such walks, 517M visited objects in 30 s. A recursive call answered by its running activation joins the activation's footprint only where a structural diff of the two states differs (rows, slots, element keys, lengths, requirements), instead of looking up every footprint entry at every call.\n\nGolden JSON and the `work:` line carry `visits` (heap entries visited by liveness walks and footprint joins, deterministic); directive `#@visits <= N`. y462 (new: a recursive walk over unknown values in a module with a 400-record table, 8.3M visits before, under 100k now), y463 (new: a recursion whose footprint is 150 records, 16k visits before, under 15k now); each fails with its switch off. ROWAN_NO=unkwalk / fpdiff: off.\n\nBench: rich in 60 s without deadlines runs 1.43M statements (0.91M before), 218M visits (736M), 285k objects (420k). At work=550000 (units count walk visits, so the budget now buys more statements): stdlib12k proven 1601 -> 2384, schema 424 -> 472; email proven 365 -> 374; rich schema 77 -> 81, 5.6 s -> 138 s and 3 GB (1.33M statements instead of 46k). Snapshots reviewed: verdicts renamed and two schema entries more (no new `missing`). Golden 1042/1042, oracle 880/880, cargo test pass. AGENTS.md: measurements and known problems as they stand.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T13:19:50+01:00",
          "tree_id": "f9b4b652a9fb33dcd8d7fe6c8b8f20f3a1f8bc40",
          "url": "https://github.com/kaidaniel/rowan/commit/587fad44c62c19018a4b524cfab7cf1d75c07cb9"
        },
        "date": 1791723176608,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1807.23,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1189,
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
            "value": 8088,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 90.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2126,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1251443,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 12650939,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 34481,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 191973,
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
          "id": "6bb839c1b16ac7555658f0b2c89a8b927280d1a7",
          "message": "Soundness: what a callee did before it raised is in the caller's handler; a with statement is a try whose finally is the managers' `__exit__` (run however the body ends: return, break / continue, an exception); break / continue run the finally blocks they leave; a `return` inside `with` on a `@contextmanager` returns from the function; `\"k%d\" % k` with `k` one of several literals is each of them formatted.\n\n* Raises from summaries: a summary keeps what its raising paths change in objects from before the call (`raise_fp`: keys from a structural diff against its entry at each raise leaving the function, after finally blocks / `__exit__`). A caller's handler starts from the state before the call with those objects weakened (rows mutated, slots joined with unknown); a call under a try that catches it runs per call site instead, within a budget (`ROWAN_LIMITS=raisetab=16` per function code). It started from the state before the call: the callee's effects were lost (`d[\"a\"]` definitely missing after `f` set it and raised).\n* `Finally` (a block, or `__exit__` of with managers) and `stmt_try_parts`: try and with share the machinery; `finally_stack` holds the running finally blocks for break / continue (saved with `loop_stack` in summary runs).\n* `CmHook` keeps the caller's exits and return values from the with body (`body_exits`, `body_ret`).\n* `format_percent_alternatives`: every combination of literal arguments (at most 64), exactly as Python formats them (`\"%s-%s\" % (\"a\", k)` was `0 | 1`).\n\ny464-y468 (new, expectations by hand; the oracle agrees; each failed before; y466's second call was corrected before commit: with `[0, 0]` `kept` is never set). ROWAN_NO=percentalts / cmreturn / jumpfinally / withexit / raisefp / raisetab: off.\n\nBench, work=550000 (loaded machine): email proven 374 -> 359, 6 s -> 8 s alone; stdlib12k proven 2384 -> 1760 (747 verdicts move to other call chains under the budget, 59 become possibly); rich unchanged. Golden 1047/1047, oracle 885/885, cargo test, snapshots (no new `missing`), hover test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T14:06:03+01:00",
          "tree_id": "1a63593b926ed11d0bb27f3da83defece48ac8fd",
          "url": "https://github.com/kaidaniel/rowan/commit/6bb839c1b16ac7555658f0b2c89a8b927280d1a7"
        },
        "date": 1791725963468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1804.88,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1494,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.23,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 6740,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 356.88,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2086,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 2753008,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 42967931,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 38926,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 272634,
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
          "id": "12002d4955ece26db027d603c45a17d9d5d5b378",
          "message": "Soundness: `rowan serve` starts every analysis with fresh thread state (the merged-object set, which made objects of a new analysis weak by the ids of an earlier one, the shape tables keyed by object ids, the cost counters); a call-cache hit replays what the run did more faithfully: deletions of rows / slots / element keys, length changes on their own, the whole facts of what the call made and its result reaches (they may have been collected meanwhile), lists' last elements cleared, an unreachable state after a run that never completes, and no hit for a run that raised while a try is active (its handlers need the state at each raise); deep call contexts are hashed on 64 bits (32 could merge two activations' strong objects).\n\nServe checked by hand (a torch case, another case, the first again: equal to the CLI's --facts); unit test merged_reset_between_analyses. The cache fixes were read in code: no small program reproducing them was found (hits need a run per call site repeated with the same observable input). ROWAN_NO=cacheborn / cacheraise / cachecompletes: off. The `\"print\"` allowlist entry is matched as `builtins.print` (the suggested-changes item was wrong; removed).\n\nBench, work=550000: stdlib12k's four false `Decimal._exp` / `_sign` / `_int` / `_is_special` missing are gone (coverage moved; no new `missing`; proven 1760 -> 1037 as cache hits under trys now run); email / rich unchanged. Golden 1047/1047, oracle 885/885, cargo test 25/25, snapshots (no new `missing`) pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T14:21:08+01:00",
          "tree_id": "8923e3bebd23c0683851d63ae63ea15d4924ab2b",
          "url": "https://github.com/kaidaniel/rowan/commit/12002d4955ece26db027d603c45a17d9d5d5b378"
        },
        "date": 1791726860698,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1805.23,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1580,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.33,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 6297,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 1079.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1387,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 4224694,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 82949019,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 53341,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 284160,
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
          "id": "263990755f7b1e22a399e948343bdc4de2dcbb3b",
          "message": "Soundness: a receiver that is one of several objects, one of whose classes runs user code for the operation (`__getitem__` / `__setitem__` / `__delitem__`, a property or descriptor, `__getattr__`, `__setattr__`), runs it per object on its own state, joined (`split_dispatch` / `per_receiver` at the entry of load_item / store_item / delete_item / load_attr / store_attr); several definitions of one `__call__` / `forward` likewise (`call_alternatives`). They ran in sequence on one state: the second saw the first's effects as definite, and the first's raise ended every path.\n\ny469 (new, expectations by hand; the oracle agrees; failed before: `log` held both alternatives' keys as definite, and the statements after a subscript one of whose `__getitem__` raises were \"never executed\"). ROWAN_NO=splitdispatch: off.\n\nBench, work=550000: unchanged but rich proven 595 -> 610. Snapshots: proto_domain verdicts renamed, no new `missing`. Golden 1048/1048, oracle 886/886, cargo test pass.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T14:29:19+01:00",
          "tree_id": "a935f003fbca7cdaef2ad1c5ca21f6a28fd6e6a8",
          "url": "https://github.com/kaidaniel/rowan/commit/263990755f7b1e22a399e948343bdc4de2dcbb3b"
        },
        "date": 1791727378481,
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
            "value": 1578,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1807.33,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 5825,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 1402.88,
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
            "value": 4224694,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 82949735,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 53341,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 284173,
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
          "id": "7f509cd81c089e448f803ba55cd03ff74df4d765",
          "message": "Bench: 10 minute timeout per program, job limit 30 minutes\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T16:22:04+01:00",
          "tree_id": "d2b1e77615af68db031b4f3834d16601b395e245",
          "url": "https://github.com/kaidaniel/rowan/commit/7f509cd81c089e448f803ba55cd03ff74df4d765"
        },
        "date": 1791732929720,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 602.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1538,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 603.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 4568,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 601.02,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 876,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
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
          "id": "215917723c2785c9faebf6b4dcf10113fb6194ae",
          "message": "SUBAGENTS.md: instructions for a subagent making one iteration of improvements (objectives, workflow on its own branch with a draft PR for benchmarks, final summary).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T16:27:02+01:00",
          "tree_id": "4d0806385438dbb1ab57d8eb533b073502f3782b",
          "url": "https://github.com/kaidaniel/rowan/commit/215917723c2785c9faebf6b4dcf10113fb6194ae"
        },
        "date": 1791733178219,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 602.12,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1539,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 602.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 5708,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 601.12,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1068,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
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
          "id": "87e184d06fab6a959eb1d4977a2360b2d4010bb4",
          "message": "Docs: one engineering document. SPECIFICATION.md is folded into AGENTS.md as a short overview of how the engine works (values, objects, state, rows, statements, calls, generators, requirements, verdicts, models, deadlines, soundness), brought up to date (with as try/finally, raises tabulating at a caller's handler, code-keyed budgets); its user-facing parts (what is modelled, what is sound) go to README.md. AGENTS.md is trimmed (191 lines) and source comments no longer cite spec sections.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T16:06:46+01:00",
          "tree_id": "dadf1f0eae454f5c2201cf335ea6824f25a3e0f1",
          "url": "https://github.com/kaidaniel/rowan/commit/87e184d06fab6a959eb1d4977a2360b2d4010bb4"
        },
        "date": 1791733230625,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 1807.28,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1577,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 1805.93,
            "unit": "s",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 8495,
            "unit": "MB",
            "extra": "cut at the 1800 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 1118.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 1375,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 4224694,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 82949735,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 53341,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 284173,
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
          "id": "76aea5462c346d8ea568cce31b1655c963d4ccf8",
          "message": "SUBAGENTS.md: the subagent chooses the kind of work (bug fixes, iterative performance work, large refactors and redesigns); the test suite catches many regressions.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-11T16:31:01+01:00",
          "tree_id": "c605fd8768f3d424174128613cac552bca856c6d",
          "url": "https://github.com/kaidaniel/rowan/commit/76aea5462c346d8ea568cce31b1655c963d4ccf8"
        },
        "date": 1791733439607,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 602.17,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1536,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 603.12,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 4274,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 601.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 870,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
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
          "id": "271bedbb4cffa966a6748bf9f7c6712dd236fa57",
          "message": "tweak",
          "timestamp": "2026-10-11T16:31:35+01:00",
          "tree_id": "5e456c270e97acd1fb658f258a507d29a8128ae1",
          "url": "https://github.com/kaidaniel/rowan/commit/271bedbb4cffa966a6748bf9f7c6712dd236fa57"
        },
        "date": 1791733463248,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 602.17,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "email: peak memory",
            "value": 1537,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: wall time",
            "value": 602.42,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 4766,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 601.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 876,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e50b933ef0feabfc5a954c62f0fb8430a74a86a5",
          "message": "Merge pull request #1 from kaidaniel/iter/bounded-cost\n\nIteration: bounded cost, fewer knobs, soundness fixes",
          "timestamp": "2026-10-12T04:11:45+01:00",
          "tree_id": "506b4bad1a747a6e97eb88fc72982d03b2bdc324",
          "url": "https://github.com/kaidaniel/rowan/commit/e50b933ef0feabfc5a954c62f0fb8430a74a86a5"
        },
        "date": 1791775478470,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 195.72,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 1176255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 15847576,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 19371,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 153556,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 602.17,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 8608,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 89.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2082,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1409718,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 1991048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36410,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 173724,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "f1770434ae0d6aaec94efa71e8ef9fb91ca78ead",
          "message": "Merge pull request #2 from kaidaniel/iter/soundness-fixes\n\nSoundness fixes: dict.update, zip, comprehension filters, view(-1), open Counter, __getattr__, requirements",
          "timestamp": "2026-10-12T08:11:10+01:00",
          "tree_id": "de7d9dab03a5486a18ff32daecc67195e627ed95",
          "url": "https://github.com/kaidaniel/rowan/commit/f1770434ae0d6aaec94efa71e8ef9fb91ca78ead"
        },
        "date": 1791789826336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 197.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 1176255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 15847576,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 19371,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 153556,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 602.07,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 10145,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 77.9,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2083,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1409718,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 1991048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36410,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 173724,
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
          "id": "53a632bed1e183dfca8effe5f4998461e1051287",
          "message": "SUBAGENTS.md: the golden pass count with every call summarised (tab=0) is part of the soundness objective, and reported before and after.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-12T09:08:27+01:00",
          "tree_id": "dd79876f6115c79f6c1c63bde205b2af26e06053",
          "url": "https://github.com/kaidaniel/rowan/commit/53a632bed1e183dfca8effe5f4998461e1051287"
        },
        "date": 1791793279178,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 210.78,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 1176255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 15847576,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 19371,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 153556,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 603.02,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 6407,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 75.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2083,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1409718,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 1991048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36410,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 173724,
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
          "id": "bee59d4cb3ea34d3f7c826be8b9ce2d973ead342",
          "message": "SUBAGENTS.md: every iteration improves what users read (hovers, report) at least once.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-12T09:15:34+01:00",
          "tree_id": "a127d9561cdb3013624076e8279801951286908d",
          "url": "https://github.com/kaidaniel/rowan/commit/bee59d4cb3ea34d3f7c826be8b9ce2d973ead342"
        },
        "date": 1791793701677,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 205.81,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 1176255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 15847576,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 19371,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 153556,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 601.97,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 10206,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 59.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2083,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1409718,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 1991048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36410,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 173724,
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
          "id": "fb420fdb1a598a7feae91d6f9bf8ed88528d382d",
          "message": "SUBAGENTS.md: every iteration leaves the engine smaller than it found it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-12T09:46:43+01:00",
          "tree_id": "d81dbaad51318dd9bb9fd3634562201d9594c7f7",
          "url": "https://github.com/kaidaniel/rowan/commit/fb420fdb1a598a7feae91d6f9bf8ed88528d382d"
        },
        "date": 1791795549952,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 225.75,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 1794,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 1176255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 15847576,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 19371,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 153556,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 602.17,
            "unit": "s",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "rich: peak memory",
            "value": 6485,
            "unit": "MB",
            "extra": "cut at the 600 s timeout"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 94.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 2083,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 1409718,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 1991048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 36410,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 173724,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "85e591da318d11b515867e7bd410234c4fe9bbcf",
          "message": "Merge pull request #3 from kaidaniel/iter/shrink-engine\n\nShrink the engine: per-call-site analysis with a bounded budget",
          "timestamp": "2026-10-12T11:24:26+01:00",
          "tree_id": "2973de9d1ae2fdefe462ad82229bd065e0655e27",
          "url": "https://github.com/kaidaniel/rowan/commit/85e591da318d11b515867e7bd410234c4fe9bbcf"
        },
        "date": 1791800832834,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 5.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 81,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 80224,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 899051,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 14464,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 8619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 605,
            "unit": "cases",
            "extra": "471 passing"
          },
          {
            "name": "rich: wall time",
            "value": 14.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 264,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 167700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 409571,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 26275,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 26055,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 16.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 417581,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 667893,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31487,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "feaaa8dd9e7447bd9cc07222c8bda4c14b26695d",
          "message": "Merge pull request #4 from kaidaniel/iter/readable-output\n\nReadable hovers and CLI report",
          "timestamp": "2026-10-12T12:58:26+01:00",
          "tree_id": "392dbb30789816d11b6de34c7e1b10a916f08490",
          "url": "https://github.com/kaidaniel/rowan/commit/feaaa8dd9e7447bd9cc07222c8bda4c14b26695d"
        },
        "date": 1791806494451,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 6.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 81,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 80224,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 899051,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 14464,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 8619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 606,
            "unit": "cases",
            "extra": "472 passing"
          },
          {
            "name": "rich: wall time",
            "value": 13.94,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 266,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 167700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 409571,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 26275,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 26055,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 13.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 416838,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 661686,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69473,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31403,
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
          "id": "0e0c26ba82cedf0f91ea46422f7d0eac9395fc52",
          "message": "smaller readme",
          "timestamp": "2026-10-12T13:30:46+01:00",
          "tree_id": "2e2d5d608825cc317898fd8b600bde5983f8a896",
          "url": "https://github.com/kaidaniel/rowan/commit/0e0c26ba82cedf0f91ea46422f7d0eac9395fc52"
        },
        "date": 1791808453545,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 7.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 82,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 80224,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 899051,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 14464,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 8619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 606,
            "unit": "cases",
            "extra": "472 passing"
          },
          {
            "name": "rich: wall time",
            "value": 12.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 266,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 167700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 409571,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 26275,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 26055,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 17.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 416838,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 661686,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69473,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31403,
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
          "id": "a8f4026b659aa9cde5b1f2053d9d4f7be7e25be5",
          "message": "Release smoke test: match the report's new wording for a missing key\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-12T14:40:14+01:00",
          "tree_id": "e33b3b2fbba519aca8b0fafebc16791831215a58",
          "url": "https://github.com/kaidaniel/rowan/commit/a8f4026b659aa9cde5b1f2053d9d4f7be7e25be5"
        },
        "date": 1791812598942,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 6.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 83,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 80224,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 899051,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 14464,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 8619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 606,
            "unit": "cases",
            "extra": "472 passing"
          },
          {
            "name": "rich: wall time",
            "value": 10.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 167700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 409571,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 26275,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 26055,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 17.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 107,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 416838,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 661686,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69473,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31403,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e0aeb5507339582d35d6f6871878f555826c5222",
          "message": "Merge pull request #5 from kaidaniel/iter/call-cache-and-type\n\nIteration 5: sound skipped calls, no call cache, type() with bases, last-round verdicts",
          "timestamp": "2026-10-13T20:44:21+01:00",
          "tree_id": "46735a3270002718c65add49960d4a3ee23c2eeb",
          "url": "https://github.com/kaidaniel/rowan/commit/e0aeb5507339582d35d6f6871878f555826c5222"
        },
        "date": 1791920829850,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "email: wall time",
            "value": 9.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 253323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 401610,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 39199,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 21241,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 613,
            "unit": "cases",
            "extra": "473 passing"
          },
          {
            "name": "rich: wall time",
            "value": 9.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 150,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 172855,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 426735,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 27817,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 25523,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 418673,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 659089,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70444,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31601,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "46a29a319185b4c2814083c9ea53002ffa135f93",
          "message": "Merge pull request #6 from kaidaniel/iter/unify-engine\n\nIteration 6: one try context, raise states everywhere, and engine cleanups",
          "timestamp": "2026-10-13T23:26:43+01:00",
          "tree_id": "c09db25216142904507463bf205de4d4fe1f5c23",
          "url": "https://github.com/kaidaniel/rowan/commit/46a29a319185b4c2814083c9ea53002ffa135f93"
        },
        "date": 1791930783334,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 210.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 207,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 366568,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 802398,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 91302,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 43106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 64,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 260258,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 305037,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 40278,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22707,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 615,
            "unit": "cases",
            "extra": "478 passing"
          },
          {
            "name": "rich: wall time",
            "value": 7.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 77,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 152856,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 256393,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 22148,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21892,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 12.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 82,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 429682,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 535221,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 72043,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 33128,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "90c918f99cf151a6dfa14c02862958e2e9e17fa6",
          "message": "Merge pull request #7 from kaidaniel/iter/big-bench\n\nIteration 7: a frontier of large benchmarks (pip, timm, sktime, functional) and optimisations against them",
          "timestamp": "2026-10-14T02:41:43+01:00",
          "tree_id": "6489a8a15a9eb50e6eccf8eae6431b3847e59fec",
          "url": "https://github.com/kaidaniel/rowan/commit/90c918f99cf151a6dfa14c02862958e2e9e17fa6"
        },
        "date": 1791942466358,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 20.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 109,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 395893,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 436602,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97045,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 25235,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 65,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 259730,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 286871,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 39853,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22968,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 362,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 276213,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 302365,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 52638,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 69769,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 620,
            "unit": "cases",
            "extra": "491 passing"
          },
          {
            "name": "pip: wall time",
            "value": 36.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 212,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 611247,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 646642,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 127567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 45129,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 134909,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 144798,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 20232,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24716,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 190.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 710,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 348732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1130828,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 88242,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 235635,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 12.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 82,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 427398,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 448303,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 72003,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31592,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 61.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 1240,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 662833,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 740736,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 129115,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 240738,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "734b13d8cf03e3e54897eaca0fa5250f013c3838",
          "message": "Merge pull request #8 from kaidaniel/iter/unify\n\nIteration 8: soundness of builtins and recursion; engine simplifications",
          "timestamp": "2026-10-14T07:48:27+01:00",
          "tree_id": "b1cfe50356b63cef6ea45f1bb256eb1b7128d34b",
          "url": "https://github.com/kaidaniel/rowan/commit/734b13d8cf03e3e54897eaca0fa5250f013c3838"
        },
        "date": 1791960934138,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 42.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 136,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 395536,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 447610,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 94989,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33718,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282568,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 310057,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22887,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.28,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 459,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 311080,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 59331,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 67456,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 623,
            "unit": "cases",
            "extra": "498 passing"
          },
          {
            "name": "pip: wall time",
            "value": 50.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 256,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 663382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 702898,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 143362,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 55375,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 12.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 128,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 136027,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 150762,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 33064,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 273.79,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 871,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378089,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1133099,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94353,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 283485,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386887,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 407201,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61778,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 32374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 56.89,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 1271,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 695514,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 770334,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 126415,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 244243,
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
          "id": "57029c0c9c4a28297bf738a9298c2492a057e4ca",
          "message": "SUBAGENTS.md: line count is a goal, not a gate; report ideas not pursued\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T09:47:13+01:00",
          "tree_id": "a19f27c094b753150b75ce7b95b0583c8b4c8ac0",
          "url": "https://github.com/kaidaniel/rowan/commit/57029c0c9c4a28297bf738a9298c2492a057e4ca"
        },
        "date": 1791968041786,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 136,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 395536,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 447610,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 94989,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33718,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 13.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282568,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 310057,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22887,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 461,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 311080,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 59331,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 67456,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 623,
            "unit": "cases",
            "extra": "498 passing"
          },
          {
            "name": "pip: wall time",
            "value": 54.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 255,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 663382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 702898,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 143362,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 55375,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 10.99,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 127,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 136027,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 150762,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 33064,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 269.76,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 871,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378089,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1133099,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94353,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 283485,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386887,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 407201,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61778,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 32374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 68.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 1268,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 695514,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 770334,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 126415,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 244243,
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
          "id": "b8ef16f5c04d19e8aa5d605782d3795965f634fd",
          "message": "SUBAGENTS.md: bring up to date (budget=0 count, eight benchmarks, guard, line counts)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T09:50:23+01:00",
          "tree_id": "1e77a03f352af007819d0693b29663bcd1300b6b",
          "url": "https://github.com/kaidaniel/rowan/commit/b8ef16f5c04d19e8aa5d605782d3795965f634fd"
        },
        "date": 1791968255158,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 37.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 136,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 395536,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 447610,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 94989,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33718,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282568,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 310057,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22887,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 459,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285700,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 311080,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 59331,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 67456,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 623,
            "unit": "cases",
            "extra": "498 passing"
          },
          {
            "name": "pip: wall time",
            "value": 50.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 255,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 663382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 702898,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 143362,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 55375,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 127,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 136027,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 150762,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 33064,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 266.76,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 871,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378089,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1133099,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94353,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 283485,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386887,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 407201,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61778,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 32374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 78.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 1268,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 695514,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 770334,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 126415,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 244243,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b00721704d2861dc202355500f01b02f94985430",
          "message": "Merge pull request #9 from kaidaniel/iter/dunders\n\nIteration 9: small-program cost, closure memory, engine size",
          "timestamp": "2026-10-14T10:25:01+01:00",
          "tree_id": "c200ec21fefaffa5944a2f999e605c7d94df454b",
          "url": "https://github.com/kaidaniel/rowan/commit/b00721704d2861dc202355500f01b02f94985430"
        },
        "date": 1791970282590,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 25.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 91,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 387019,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 418092,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 91762,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 20442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 9.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 68,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283604,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311460,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49895,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22503,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 278,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285821,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 313989,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 60161,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 65890,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 625,
            "unit": "cases",
            "extra": "504 passing"
          },
          {
            "name": "pip: wall time",
            "value": 37.9,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 257,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 637352,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 672164,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 138066,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 46249,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 8.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 135693,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 145538,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 20855,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24413,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 239.73,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 749,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378089,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1121584,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94353,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 282913,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386579,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 406482,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61669,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30921,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 66.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 690,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 708005,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 785048,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 130300,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 245939,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4aeb5eea40ae8e1d3dd0355b717f7c9092066570",
          "message": "Merge pull request #10 from kaidaniel/iter/vsix-windows-playground\n\nIteration 10: packed-.vsix test, Windows target, pytorch-playground cases",
          "timestamp": "2026-10-14T11:57:16+01:00",
          "tree_id": "ab0c71e7f81edcbe4d612d867d6c48e443c48c85",
          "url": "https://github.com/kaidaniel/rowan/commit/4aeb5eea40ae8e1d3dd0355b717f7c9092066570"
        },
        "date": 1791975879907,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 91,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 387019,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 418092,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 91762,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 20442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 14.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 68,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283604,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311460,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49895,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22503,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 12.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 275,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285793,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 313720,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 60158,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 65312,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 632,
            "unit": "cases",
            "extra": "509 passing"
          },
          {
            "name": "pip: wall time",
            "value": 45.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 256,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 637352,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 672164,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 138066,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 46250,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 132083,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 140860,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 20430,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 19586,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 278.89,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 758,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1121904,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94514,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 283480,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386579,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 406482,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61669,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30920,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 47.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 690,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 708048,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 785121,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 130301,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 245691,
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
          "id": "9b61c832c755e43d97c1f168e787c07c3ce7b1a2",
          "message": "SUBAGENTS.md: open #@todo count as an objective, reported before and after\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T12:21:11+01:00",
          "tree_id": "0753cca7199947fe85858cc3873621ccb0ca4b3f",
          "url": "https://github.com/kaidaniel/rowan/commit/9b61c832c755e43d97c1f168e787c07c3ce7b1a2"
        },
        "date": 1791977302980,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 91,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 387019,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 418092,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 91762,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 20442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 68,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283604,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311460,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49895,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22503,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 12.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 275,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 285793,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 313720,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 60158,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 65312,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 632,
            "unit": "cases",
            "extra": "509 passing"
          },
          {
            "name": "pip: wall time",
            "value": 47.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 257,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 637352,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 672164,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 138066,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 46250,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 8.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 107,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 132083,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 140860,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 20430,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 19586,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 275.85,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 759,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 378365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 1121904,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 94514,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 283480,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386579,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 406482,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61669,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30920,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 43.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 691,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 708048,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 785121,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 130301,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 245691,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "65776d6480273072424cbfbc7d30401fb475608e",
          "message": "Merge pull request #11 from kaidaniel/iter/design-unify\n\nIteration 11: tests evaluated once (split); sound generators, recursion, isinstance",
          "timestamp": "2026-10-14T21:45:04+01:00",
          "tree_id": "2ca1c481cd4a8c426b7cdc1af360e031a8f66944",
          "url": "https://github.com/kaidaniel/rowan/commit/65776d6480273072424cbfbc7d30401fb475608e"
        },
        "date": 1792011058121,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 18.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 79,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 394779,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 430822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 92049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 19936,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 13.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 66,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311719,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48952,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23160,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 253,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 435662,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 628094,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 95985,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98740,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 633,
            "unit": "cases",
            "extra": "514 passing"
          },
          {
            "name": "pip: wall time",
            "value": 27.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 230,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 630094,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662999,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133602,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44113,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 8.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138424,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151415,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21297,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 19637,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 205.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 947,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 386758,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 974145,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95196,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 282720,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386150,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401736,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61440,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31286,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 103.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 877,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891176,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165518,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 405108,
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
          "id": "144e088f02bfdce663e06025aa6240a1b20d408e",
          "message": "SUBAGENTS.md: SUGGESTED_CHANGES lists only open avenues, by topic, not a changelog\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T23:09:45+01:00",
          "tree_id": "9c3fa247c1b5269f711f8686f3995e4e6b8df76b",
          "url": "https://github.com/kaidaniel/rowan/commit/144e088f02bfdce663e06025aa6240a1b20d408e"
        },
        "date": 1792016147113,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 30.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 77,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 394779,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 430822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 92049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 19936,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 14.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 67,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311719,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48952,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23160,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 254,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 435662,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 628094,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 95985,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98740,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 633,
            "unit": "cases",
            "extra": "514 passing"
          },
          {
            "name": "pip: wall time",
            "value": 36.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 231,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 630094,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662999,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133602,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44113,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138424,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151415,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21297,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 19637,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 203.83,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 947,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 386758,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 974145,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95196,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 282720,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 85,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386150,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401736,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61440,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31286,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 106.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 879,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891176,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165518,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 405108,
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
          "id": "1eeb59c31f57d136fae87b022b149738d82520b7",
          "message": "SUBAGENTS.md: modular, readable code as objective 10, with proxies reported\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T23:17:29+01:00",
          "tree_id": "7b48a740beba2142991364442c9bfb3d05c469f2",
          "url": "https://github.com/kaidaniel/rowan/commit/1eeb59c31f57d136fae87b022b149738d82520b7"
        },
        "date": 1792016540952,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 79,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 394779,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 430822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 92049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 19936,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 67,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311719,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48952,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23160,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 15.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 251,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 435662,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 628094,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 95985,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98740,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 633,
            "unit": "cases",
            "extra": "514 passing"
          },
          {
            "name": "pip: wall time",
            "value": 35.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 231,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 630094,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662999,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133602,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44113,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138424,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151415,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21297,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 19637,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 141.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 947,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 386758,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 974145,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95196,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 282720,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 85,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386150,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401736,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61440,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31286,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 110.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 883,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891176,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165518,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 405108,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "efeee3c7960c18c26dd085be4e2726883ce68450",
          "message": "Merge pull request #12 from kaidaniel/iter/todos\n\nIteration 12: work down the #@todo expectations",
          "timestamp": "2026-10-15T00:15:52+01:00",
          "tree_id": "44e1b9d6e1b0febecfbb8ffd065745bfcb9e67da",
          "url": "https://github.com/kaidaniel/rowan/commit/efeee3c7960c18c26dd085be4e2726883ce68450"
        },
        "date": 1792020107006,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 30.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 78,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 394952,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 431074,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 92049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 19937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 67,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311719,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48952,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23160,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 24.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 252,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 441337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 637563,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 96093,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99581,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 633,
            "unit": "cases",
            "extra": "519 passing"
          },
          {
            "name": "pip: wall time",
            "value": 27.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 231,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 629591,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662536,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133452,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44189,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 7.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138644,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151755,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21333,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20111,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 209.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 946,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 386564,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 971153,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 280685,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 7.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386009,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401756,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61428,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31568,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 104.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 878,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 835659,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 890393,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165766,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 399156,
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bf6f82ac6f3527d28a7f40b0bd83d42067abc8a5",
          "message": "Merge pull request #13 from kaidaniel/iter/core-research\n\nResearch: where rowan's time and memory go; mimalloc, allocation-free empty sets",
          "timestamp": "2026-10-15T01:47:04+01:00",
          "tree_id": "2d8157219a0eb6f80c2db915fc522af729499d39",
          "url": "https://github.com/kaidaniel/rowan/commit/bf6f82ac6f3527d28a7f40b0bd83d42067abc8a5"
        },
        "date": 1792025490451,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 103,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 394952,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 431074,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 92049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 19937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 93,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 282055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311719,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48952,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23160,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 20.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 274,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 441337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 637563,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 96093,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99581,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 633,
            "unit": "cases",
            "extra": "519 passing"
          },
          {
            "name": "pip: wall time",
            "value": 18.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 255,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 629591,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662536,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133452,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44189,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 142,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138644,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151755,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21333,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20111,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 96.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 798,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 386564,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 971153,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 280685,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 123,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386009,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401756,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61428,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31568,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 81.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 893,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 835659,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 890393,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165766,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 399156,
            "unit": "count",
            "extra": "finished"
          }
        ]
      }
    ]
  }
}