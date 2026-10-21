window.BENCHMARK_DATA = {
  "lastUpdate": 1792599058652,
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
          "id": "dfbf75b4ff1a3a9c18fa5c50b882cdb436837c55",
          "message": "Merge pull request #14 from kaidaniel/iter/modularity\n\nModularity: split Analyzer into components with narrow interfaces (no behaviour change)",
          "timestamp": "2026-10-15T04:19:41+01:00",
          "tree_id": "2267732e785b39e19ded8f1de8ff8f65ac3dd93d",
          "url": "https://github.com/kaidaniel/rowan/commit/dfbf75b4ff1a3a9c18fa5c50b882cdb436837c55"
        },
        "date": 1792034709190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.82,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 100,
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
            "value": 91,
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
            "value": 16.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 271,
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
            "value": 16.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 254,
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
            "value": 5.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 144,
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
            "value": 165.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 796,
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
            "value": 9.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 124,
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
            "value": 48.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 898,
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
          "id": "04ae0a487c523a105c9781302ebc58c816549a1c",
          "message": "Merge pull request #15 from kaidaniel/iter/call-memo\n\nIteration 15: call memo measured and ruled out; loop-return and slice soundness fixes",
          "timestamp": "2026-10-15T06:01:09+01:00",
          "tree_id": "9e59faff57c9db0cc39d8b21bbbdf7b65ec3746b",
          "url": "https://github.com/kaidaniel/rowan/commit/04ae0a487c523a105c9781302ebc58c816549a1c"
        },
        "date": 1792040850158,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 26.48,
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
            "value": 397684,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 434367,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 93122,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 20090,
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
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283192,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312813,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49475,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23530,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 269,
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
            "value": 638395,
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
            "value": 99619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 637,
            "unit": "cases",
            "extra": "522 passing"
          },
          {
            "name": "pip: wall time",
            "value": 31.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 249,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 629153,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 662034,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 133342,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 44114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 7.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 141,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138335,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151376,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21290,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20109,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 217.84,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 1040,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 387632,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 841664,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95784,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 265159,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 386057,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 401673,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 61443,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31541,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 83.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 899,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 835670,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 890407,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165776,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 399045,
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
          "id": "7fdb20924c468c84d20d073537bdb341f29a58aa",
          "message": "Merge pull request #16 from kaidaniel/iter/entry-order\n\nIteration 16: entry runs independent of their order; sktime cost",
          "timestamp": "2026-10-15T08:19:31+01:00",
          "tree_id": "ae191c12b98c0d812a7f693e61550034284487d8",
          "url": "https://github.com/kaidaniel/rowan/commit/7fdb20924c468c84d20d073537bdb341f29a58aa"
        },
        "date": 1792049010007,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.07,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427647,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 465041,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99134,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 27046,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312610,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49488,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23539,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 264,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 394650,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 468475,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 93970,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 92538,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 639,
            "unit": "cases",
            "extra": "531 passing"
          },
          {
            "name": "pip: wall time",
            "value": 23.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 264,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 744762,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 785653,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 153775,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 65135,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 6.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 139,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 138335,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 151375,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 21290,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 103.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 544,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 387555,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 861750,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 249606,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 389708,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 404461,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 62028,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30981,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 81.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 895,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 835670,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 890394,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 165776,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 399045,
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
          "id": "5669533e78b77ef64359b781c08fe5ada0c6be61",
          "message": "Merge pull request #17 from kaidaniel/iter/gpt-budget\n\nIteration 17: GPT heads within budget (summary layers), comprehension objects as summaries, picked-callee verdicts",
          "timestamp": "2026-10-15T10:31:35+01:00",
          "tree_id": "a852c2f12952bc221ba8fc85103f3f071e816cb7",
          "url": "https://github.com/kaidaniel/rowan/commit/5669533e78b77ef64359b781c08fe5ada0c6be61"
        },
        "date": 1792056956365,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.73,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 107,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427660,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 463739,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99266,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 27512,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.54,
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
            "value": 283356,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312678,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 50028,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23413,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 15.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 262,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 469508,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 97893,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 96596,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 639,
            "unit": "cases",
            "extra": "535 passing"
          },
          {
            "name": "pip: wall time",
            "value": 34.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 279,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 736143,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 788141,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 151907,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 67089,
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
            "value": 139,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 141505,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 154825,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 22169,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22963,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 98.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 539,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 387425,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 692931,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 250018,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 119,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 389610,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 404592,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 62098,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30911,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 44.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 768,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834729,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 886681,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 166680,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 272003,
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
          "id": "0b42a1172ea435ae7cc41f05ad29bcbb19ca14e8",
          "message": "Merge pull request #18 from kaidaniel/iter/loop-items\n\nLoop items as views of their summary (soundness)",
          "timestamp": "2026-10-15T12:51:09+01:00",
          "tree_id": "6453898e86d9e372dfc9a426c21becb0851fc582",
          "url": "https://github.com/kaidaniel/rowan/commit/0b42a1172ea435ae7cc41f05ad29bcbb19ca14e8"
        },
        "date": 1792065279962,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.52,
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
            "value": 428055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464240,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99727,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28283,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 95,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313041,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 50029,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23722,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 272,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399288,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 469804,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 97894,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 97813,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 641,
            "unit": "cases",
            "extra": "537 passing"
          },
          {
            "name": "pip: wall time",
            "value": 28.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 276,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 828760,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 909097,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 175363,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 68373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 14.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 172,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 253565,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 283214,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 39333,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23780,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 89.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 546,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 387426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 692924,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 250948,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413510,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429555,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65680,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31512,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 69.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 740,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834729,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 886569,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 166680,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 273120,
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
          "id": "fde88a0972d55bbcdb45d36c37c50d56851f5f81",
          "message": "Merge pull request #19 from kaidaniel/iter/todos-2\n\nIteration 19: #@todo triage and fixes",
          "timestamp": "2026-10-15T14:42:28+01:00",
          "tree_id": "f0700df1213cd2c94580b6a6074b13158da48f14",
          "url": "https://github.com/kaidaniel/rowan/commit/fde88a0972d55bbcdb45d36c37c50d56851f5f81"
        },
        "date": 1792071997883,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 116,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464220,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99727,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28283,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 99,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283846,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313515,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 50109,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23676,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399749,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 471110,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 98125,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 97823,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 641,
            "unit": "cases",
            "extra": "540 passing"
          },
          {
            "name": "pip: wall time",
            "value": 30.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 271,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 830382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 911285,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 175723,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 68295,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 14.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 183,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 255001,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 284753,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 39656,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23357,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 93.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 542,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 387426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 692924,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 95759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 250948,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413510,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429554,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65680,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31512,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 83.75,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 735,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 834788,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 886632,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 166721,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 273221,
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
          "id": "5723bfbcf19bdcb62dfaea3729e0106858cdf0b8",
          "message": "Merge pull request #20 from kaidaniel/iter/default-escape\n\nIteration 20: escape analysis for skipped calls (defaults, aliases, class attributes)",
          "timestamp": "2026-10-15T18:29:32+01:00",
          "tree_id": "505ddd968dcf885d9750ab6b9d84909f19335c50",
          "url": "https://github.com/kaidaniel/rowan/commit/5723bfbcf19bdcb62dfaea3729e0106858cdf0b8"
        },
        "date": 1792085614554,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 15.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427114,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 463047,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99566,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 92,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283481,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313167,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49843,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23706,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 261,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 376257,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 429401,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 82893,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 97377,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 641,
            "unit": "cases",
            "extra": "552 passing"
          },
          {
            "name": "pip: wall time",
            "value": 47.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 293,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 967332,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1051787,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 200805,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88872,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 176,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 496232,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 530709,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88143,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22872,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 88.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 470,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 404129,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 687427,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 99710,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 191872,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 130,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 412817,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428842,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65573,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31448,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 62.07,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 668,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852191,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900383,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171706,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253425,
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
          "id": "0d6ecf584c1cae9eb2cc5a2b4e93869092af7d16",
          "message": "Merge pull request #21 from kaidaniel/iter/general-facts\n\nIteration 21: general and per-call facts in hovers, typed unknowns",
          "timestamp": "2026-10-15T20:52:58+01:00",
          "tree_id": "cd3637c17b08d264730dc35c8b9a88637b722280",
          "url": "https://github.com/kaidaniel/rowan/commit/0d6ecf584c1cae9eb2cc5a2b4e93869092af7d16"
        },
        "date": 1792094243806,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 20.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464447,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 642,
            "unit": "cases",
            "extra": "553 passing"
          },
          {
            "name": "pip: wall time",
            "value": 46.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 295,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 36.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 175,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 57.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 401,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65630,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31529,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 56.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 670,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "524111a7dd2a87e8140b6ca4f3c0bb642aa1d551",
          "message": "Merge pull request #23 from kaidaniel/marketplace-publishing\n\nPublish the .vsix files to the Marketplace from the Release workflow",
          "timestamp": "2026-10-15T21:35:55+01:00",
          "tree_id": "b0a163242a49225d69433983dda9e338b099bcd9",
          "url": "https://github.com/kaidaniel/rowan/commit/524111a7dd2a87e8140b6ca4f3c0bb642aa1d551"
        },
        "date": 1792096960514,
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
            "value": 111,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464447,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 264,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 642,
            "unit": "cases",
            "extra": "553 passing"
          },
          {
            "name": "pip: wall time",
            "value": 51.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 295,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 29.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 175,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 66.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 402,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65630,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31529,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 49.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 671,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "125d8df24bc7b8795bcfbd6f8de5dccd9dfcc67c",
          "message": "Merge pull request #24 from kaidaniel/marketplace-identity-fix\n\nMarketplace identity: read the id from connectionData, check publisher access",
          "timestamp": "2026-10-15T21:41:51+01:00",
          "tree_id": "7396bb3d8e3c3d4eab71337c4396c989600a66d9",
          "url": "https://github.com/kaidaniel/rowan/commit/125d8df24bc7b8795bcfbd6f8de5dccd9dfcc67c"
        },
        "date": 1792097176367,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 111,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464447,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 642,
            "unit": "cases",
            "extra": "553 passing"
          },
          {
            "name": "pip: wall time",
            "value": 31.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 294,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 23.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 178,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 65.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 401,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65630,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31529,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 56.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 666,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "58109e065abb4534d3181e69de0d5d8323740fbf",
          "message": "Merge pull request #22 from kaidaniel/iter/relational\n\nIteration 22: relational facts (research)",
          "timestamp": "2026-10-15T22:45:38+01:00",
          "tree_id": "cb7d862b8842bba55fd3d281bfa3b97422b6bdfa",
          "url": "https://github.com/kaidaniel/rowan/commit/58109e065abb4534d3181e69de0d5d8323740fbf"
        },
        "date": 1792100978271,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 13.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464447,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 261,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "556 passing"
          },
          {
            "name": "pip: wall time",
            "value": 47.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 295,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 20.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 179,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 62.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 398,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65630,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31529,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 79.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 672,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900413,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "55684be7d254a4578ddaea46903e7080bbdf560e",
          "message": "Merge pull request #25 from kaidaniel/iter/on-demand\n\nIteration 23: on-demand analysis in rowan serve",
          "timestamp": "2026-10-16T02:24:28+01:00",
          "tree_id": "aeb9adc193a37386a4e5a866a827aa0d99f23cbf",
          "url": "https://github.com/kaidaniel/rowan/commit/55684be7d254a4578ddaea46903e7080bbdf560e"
        },
        "date": 1792114104470,
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
            "value": 109,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464447,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29379,
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
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 15.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 263,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "556 passing"
          },
          {
            "name": "pip: wall time",
            "value": 46.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 294,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 31.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 179,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 44.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 401,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
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
            "value": 123,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413061,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65630,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31529,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 69.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 669,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900413,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "b45801bb3776723d2decf33e6cc071ea07b72556",
          "message": "Merge pull request #26 from kaidaniel/iter/contracts\n\nIteration 24: general runs sound for any call; parameter contracts in hovers",
          "timestamp": "2026-10-16T04:02:10+01:00",
          "tree_id": "bba163b7bdadc2e518a3bbbee0fabcc6f199cbd8",
          "url": "https://github.com/kaidaniel/rowan/commit/b45801bb3776723d2decf33e6cc071ea07b72556"
        },
        "date": 1792119951804,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 13.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464498,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29391,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 271,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 384054,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 437855,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 84114,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100826,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "558 passing"
          },
          {
            "name": "pip: wall time",
            "value": 30.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 303,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977242,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1062129,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202492,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 30.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 178,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540968,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 42.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 405,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413005,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429079,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65620,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31528,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 55.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 670,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900413,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "0c599d0bdf34183e093e1635d78bd74dca087f07",
          "message": "Merge pull request #27 from kaidaniel/iter/any-call-flows\n\nAny-call start: precise flows (returns, arguments, containers), enum members, two soundness fixes",
          "timestamp": "2026-10-16T06:39:12+01:00",
          "tree_id": "983eb60ff8b9abdaa494a14f2ef39047eef88f6d",
          "url": "https://github.com/kaidaniel/rowan/commit/0c599d0bdf34183e093e1635d78bd74dca087f07"
        },
        "date": 1792129382704,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 464498,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99156,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 29391,
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
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313219,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49837,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24147,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 20.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 316,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 385725,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 442193,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86058,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 101049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "563 passing"
          },
          {
            "name": "pip: wall time",
            "value": 33.71,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 298,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 977034,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1061919,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202452,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 93354,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 31.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 180,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 505974,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 540967,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 88937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24598,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 63.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 402,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436488,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 584992,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107072,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 153146,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413005,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429079,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65620,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31528,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 48.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 677,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 852196,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 900413,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 171708,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253433,
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
          "id": "ba3c6be10fd850e3c465d96b8a31419de9f62c56",
          "message": "Merge pull request #28 from kaidaniel/iter/ondemand-tails\n\nTime to information: on-demand tails, the first hover after the module code",
          "timestamp": "2026-10-16T09:35:50+01:00",
          "tree_id": "6a68431d8d63cad96fe355d231211c67069f9932",
          "url": "https://github.com/kaidaniel/rowan/commit/ba3c6be10fd850e3c465d96b8a31419de9f62c56"
        },
        "date": 1792139980571,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 111,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434642,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 472781,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101079,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28859,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283561,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313227,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49851,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24117,
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
            "value": 320,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 385735,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 442163,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86061,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100883,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "565 passing"
          },
          {
            "name": "pip: wall time",
            "value": 45.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 297,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 975298,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1057432,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202268,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85377,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 31.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 189,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497030,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531205,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86279,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24263,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 40.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 260,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 429223,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 540380,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 105741,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 112445,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414326,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 430311,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30795,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 56.1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 669,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 855547,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 903664,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 172199,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253117,
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
          "id": "f337ccc26203b85a258b744c6dacd82e8a60f2a0",
          "message": "Merge pull request #30 from kaidaniel/retire-suggested-changes\n\nRetire SUGGESTED_CHANGES.md; rewrite SUBAGENTS.md",
          "timestamp": "2026-10-16T11:04:03+01:00",
          "tree_id": "5cb3b385d69b80af6d9ddf5865fa77e11ea09cc9",
          "url": "https://github.com/kaidaniel/rowan/commit/f337ccc26203b85a258b744c6dacd82e8a60f2a0"
        },
        "date": 1792145286783,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 18.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 111,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434642,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 472781,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101079,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28859,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283561,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313227,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49851,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 320,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 385735,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 442163,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86061,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100883,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 643,
            "unit": "cases",
            "extra": "565 passing"
          },
          {
            "name": "pip: wall time",
            "value": 44.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 297,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 975298,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1057432,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 202268,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85377,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 29.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 189,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497030,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531205,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86279,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24263,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 38.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 261,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 429223,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 540380,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 105741,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 112445,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414326,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 430311,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30795,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 71.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 669,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 855547,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 903664,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 172199,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253117,
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
          "id": "0da9fda3f60485aeb491ffcee5bad1778b75e920",
          "message": "Merge pull request #29 from kaidaniel/iter/soundness-bugs\n\nSoundness bugs: repros and fixes (iteration 27)",
          "timestamp": "2026-10-16T11:07:12+01:00",
          "tree_id": "a6511ee2a2468ff58b01758b6cda4796ecf2e374",
          "url": "https://github.com/kaidaniel/rowan/commit/0da9fda3f60485aeb491ffcee5bad1778b75e920"
        },
        "date": 1792145453162,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434642,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 469035,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101079,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 28919,
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
            "value": 97,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283841,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311400,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49841,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24238,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 345,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403547,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 475845,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 92155,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 109430,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "574 passing"
          },
          {
            "name": "pip: wall time",
            "value": 50.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 294,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 968644,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1046533,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 200880,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83657,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.42,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 185,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498115,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521591,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86577,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24414,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 31.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 258,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 431485,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 539029,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106757,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 107666,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 412500,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 426967,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65459,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30473,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 66.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 665,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 845109,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 893145,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169451,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 253259,
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
          "id": "246674f9c1b9524b06ef7e86c85260691e3dd0a9",
          "message": "Merge pull request #31 from kaidaniel/iter/editor-tails\n\nEditor memory and time tails: wide reads share placeholders, serve cancels stale runs",
          "timestamp": "2026-10-16T14:36:49+01:00",
          "tree_id": "f29c1c4f8051c1a5bd31c4715c73653cf0a4c9c6",
          "url": "https://github.com/kaidaniel/rowan/commit/246674f9c1b9524b06ef7e86c85260691e3dd0a9"
        },
        "date": 1792158012938,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 473028,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101089,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30330,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 95,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283849,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311529,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49845,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23748,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 25.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 353,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403695,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 479491,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 92120,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 108493,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "578 passing"
          },
          {
            "name": "pip: wall time",
            "value": 45.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 292,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 971535,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1049141,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 201306,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83602,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 23.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 185,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497675,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521106,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86484,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24285,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 43.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 310,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 430479,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 502634,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106528,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 106075,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428500,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65435,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30137,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 57.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 713,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844189,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891927,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169439,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230202,
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
          "id": "e479dab203b4cdb4626bd68a78c7b56378eab7bf",
          "message": "Merge pull request #32 from kaidaniel/iter/hover-format\n\nHovers in the owner's format (name = value; Requirements / In Calls / Notes)",
          "timestamp": "2026-10-16T15:06:33+01:00",
          "tree_id": "372bd6c479e4ba729f97511ae389a58debd09bc2",
          "url": "https://github.com/kaidaniel/rowan/commit/e479dab203b4cdb4626bd68a78c7b56378eab7bf"
        },
        "date": 1792159839519,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 473028,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101089,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30330,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 95,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283849,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311529,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49845,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23748,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 25.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 353,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403695,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 479491,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 92120,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 108493,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "578 passing"
          },
          {
            "name": "pip: wall time",
            "value": 26.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 296,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 971535,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1049141,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 201306,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83602,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 28.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 185,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497675,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521106,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86484,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24285,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 42.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 310,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 430479,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 502634,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106528,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 106075,
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
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428500,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65435,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30137,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 73.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 713,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844189,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891927,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169439,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230202,
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
          "id": "6b0ca2bdbf4f8e2b10b193ebe603c7223b661724",
          "message": "Merge pull request #33 from kaidaniel/iter/requirement-paths\n\nRequirement paths through selections; class-valued call results for skipped stores",
          "timestamp": "2026-10-16T16:17:21+01:00",
          "tree_id": "d634f12da5f1546be7d38baff52255c791e8a35f",
          "url": "https://github.com/kaidaniel/rowan/commit/6b0ca2bdbf4f8e2b10b193ebe603c7223b661724"
        },
        "date": 1792164065878,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 15.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 433901,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 473042,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101089,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30389,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 95,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283854,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311572,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49845,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 21.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 339,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 385806,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 421379,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86013,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99984,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "581 passing"
          },
          {
            "name": "pip: wall time",
            "value": 33.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 292,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 971151,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1048823,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 201270,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83582,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 28.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 185,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497389,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 520799,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86475,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24248,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 48.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 311,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 430467,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 502827,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 106775,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414200,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428596,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65435,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 70.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 714,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844189,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891978,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169439,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230798,
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
          "id": "61f462a06de14d2b39afb9e45f28afb651c99856",
          "message": "Merge pull request #34 from kaidaniel/iter/hover-sections\n\nHovers: parameter types, Allocation Sites, Requirements only of parameters, Notes only what is used, cut-analysis note; typed unknowns in containers",
          "timestamp": "2026-10-16T17:08:09+01:00",
          "tree_id": "21a2e2d4d80cffcce80d5bce50bea5e3815465fe",
          "url": "https://github.com/kaidaniel/rowan/commit/61f462a06de14d2b39afb9e45f28afb651c99856"
        },
        "date": 1792167105931,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 25.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 433901,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 473042,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 101089,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30389,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 99,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283854,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311572,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 49845,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 333,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 385806,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 421379,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86013,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99984,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "581 passing"
          },
          {
            "name": "pip: wall time",
            "value": 41.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 296,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 971151,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1048823,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 201270,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83582,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 29.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 188,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497389,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 520799,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86475,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24248,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 37.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 311,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 430467,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 502827,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 106775,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414200,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428596,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 65435,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 49.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 727,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844189,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891978,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169439,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230798,
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
          "id": "d17ecf384e9ceb62afcfca78b40363b5ae25653d",
          "message": "Merge pull request #35 from kaidaniel/iter/review-32\n\nIteration 32: soundness of container values (lists, sets, texts), found by a differential fuzzer",
          "timestamp": "2026-10-16T19:55:16+01:00",
          "tree_id": "66f9d195ffbb2d6c479ba9de5592aac61af220b7",
          "url": "https://github.com/kaidaniel/rowan/commit/d17ecf384e9ceb62afcfca78b40363b5ae25653d"
        },
        "date": 1792177120780,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 15.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 440140,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 482025,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 102850,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30431,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.54,
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
            "value": 282959,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 310002,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48941,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23479,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 21.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 348,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400987,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 430566,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 86890,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98618,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 647,
            "unit": "cases",
            "extra": "592 passing"
          },
          {
            "name": "pip: wall time",
            "value": 26.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 294,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 971365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1048822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 201311,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 83608,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 23.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 191,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 500108,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 524061,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86503,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24752,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 40.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 301,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 430106,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 502471,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 106436,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 107418,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 416533,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429473,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 66004,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30175,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 50.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 726,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 845281,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892774,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 170049,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 234339,
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
          "id": "3b4fecf174e58caef224c87285dd38e0502145de",
          "message": "Merge pull request #36 from kaidaniel/iter/fuzz-areas\n\nIteration 33: fuzzing beyond lists (dicts, attributes, try, generators, formatting, calls, budgets)",
          "timestamp": "2026-10-16T23:46:01+01:00",
          "tree_id": "714becd67290ad679e3d2254818393b5a5097989",
          "url": "https://github.com/kaidaniel/rowan/commit/3b4fecf174e58caef224c87285dd38e0502145de"
        },
        "date": 1792190976806,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 137,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441254,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476844,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103872,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33512,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283451,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311484,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48951,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23429,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 357,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403912,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433115,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87450,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98711,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 648,
            "unit": "cases",
            "extra": "629 passing"
          },
          {
            "name": "pip: wall time",
            "value": 36.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 301,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1063608,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1134704,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 225032,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85601,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 27.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 172,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 512467,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 537589,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 89759,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23558,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 43.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 304,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436634,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512801,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107505,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104188,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 411150,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 424400,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64964,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30949,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 72.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 720,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 846394,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 893970,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169613,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 236311,
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
          "id": "69a760022e28966d299f95f0d0f2bb836cfb5cf5",
          "message": "Merge pull request #37 from kaidaniel/iter/req-paths\n\nRequirement paths through unknown calls, unknown list concatenation, and small open items",
          "timestamp": "2026-10-17T01:15:53+01:00",
          "tree_id": "fddc0c01e24fd13db05abdd94136b20212532d35",
          "url": "https://github.com/kaidaniel/rowan/commit/69a760022e28966d299f95f0d0f2bb836cfb5cf5"
        },
        "date": 1792196352449,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.39,
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
            "value": 441228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103868,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33516,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23651,
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
            "value": 358,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403879,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433084,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 648,
            "unit": "cases",
            "extra": "635 passing"
          },
          {
            "name": "pip: wall time",
            "value": 48.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 288,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1063154,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1134204,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 224772,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85311,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 28.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 162,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 511877,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 536911,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 89310,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23619,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 32.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 301,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436607,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512787,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104215,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408616,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421839,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64652,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30968,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 62.72,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 718,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844814,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892060,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 236289,
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
          "id": "f83330a0ed9c356fc7959c2673a38041741999a2",
          "message": "Merge pull request #38 from kaidaniel/iter/library-readers\n\nIteration 35: class attributes hidden by unknown code (soundness), library functions that only read, hovers of rebound names",
          "timestamp": "2026-10-17T02:21:15+01:00",
          "tree_id": "c1aa2a5aeb1b36b0b650fe5dd55e7558a8c43415",
          "url": "https://github.com/kaidaniel/rowan/commit/f83330a0ed9c356fc7959c2673a38041741999a2"
        },
        "date": 1792200285665,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 135,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103868,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33516,
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
            "value": 97,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23651,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 21.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 354,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403879,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433084,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 648,
            "unit": "cases",
            "extra": "638 passing"
          },
          {
            "name": "pip: wall time",
            "value": 41.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 293,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1061527,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1131906,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223705,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85149,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 27.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 162,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 508572,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532651,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87031,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23083,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 35.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 298,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436612,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512792,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408616,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421839,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64652,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30968,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 49.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 718,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844814,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892060,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 236289,
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
          "id": "9645f4463d264c1cc578a9fb31d9e7103ce2c5eb",
          "message": "Merge pull request #39 from kaidaniel/iter/readability\n\nReadability: structure, docs and READMEs (no behaviour change)",
          "timestamp": "2026-10-17T04:20:05+01:00",
          "tree_id": "ad0404f3eeb8bd53ee32e856a7049cdd992053b0",
          "url": "https://github.com/kaidaniel/rowan/commit/9645f4463d264c1cc578a9fb31d9e7103ce2c5eb"
        },
        "date": 1792207420378,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 25.53,
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
            "value": 441228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103868,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33516,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 99,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23651,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 359,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403879,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433084,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 648,
            "unit": "cases",
            "extra": "638 passing"
          },
          {
            "name": "pip: wall time",
            "value": 47.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 292,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1061527,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1131906,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223705,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85149,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 18.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 162,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 508572,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532651,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87031,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23083,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 39.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 299,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436612,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512792,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408616,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421839,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64652,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30968,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 53.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 721,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844814,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892060,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 236289,
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
          "id": "6d999a44c32cf948deea1b16261e49a19a285995",
          "message": "Merge pull request #40 from kaidaniel/iter/data-structure-research\n\nResearch: data structures for the hot paths (remembered-set GC, sharing-preserving tries; radix tries tried and reverted)",
          "timestamp": "2026-10-17T05:38:45+01:00",
          "tree_id": "08052acf6eb32eb4b50d5d5bab9b2927f5e13754",
          "url": "https://github.com/kaidaniel/rowan/commit/6d999a44c32cf948deea1b16261e49a19a285995"
        },
        "date": 1792212136003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 13.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103868,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33516,
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
            "value": 87,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23651,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 13.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 352,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403879,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433084,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87442,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 648,
            "unit": "cases",
            "extra": "638 passing"
          },
          {
            "name": "pip: wall time",
            "value": 40.8,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 266,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1061527,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1131906,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223705,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85149,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 508572,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532651,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87031,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23083,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 38.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 292,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436612,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512792,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 116,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408616,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421839,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64652,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30968,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 45.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 687,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844814,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892060,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 236289,
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
          "id": "b990fd9a972fc0f15b5beaf6be2ea1fd2632e06e",
          "message": "Merge pull request #41 from kaidaniel/iter/coverage-hovers\n\nIteration 38: hover accuracy (unknown copies, module defaults, FastAPI In Calls), deterministic deadline cut",
          "timestamp": "2026-10-17T06:22:39+01:00",
          "tree_id": "7908ce5c456b3b50b09248c1150aec688e07abb0",
          "url": "https://github.com/kaidaniel/rowan/commit/b990fd9a972fc0f15b5beaf6be2ea1fd2632e06e"
        },
        "date": 1792214771131,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441228,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 476822,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103868,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33516,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311388,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23651,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 15.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 343,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 402983,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 432205,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87339,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99487,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 649,
            "unit": "cases",
            "extra": "641 passing"
          },
          {
            "name": "pip: wall time",
            "value": 27.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 261,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1061429,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1132027,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223680,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85128,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.47,
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
            "value": 506759,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531448,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86879,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23023,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 26.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 300,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436219,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512860,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107448,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408656,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421883,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64662,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30941,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 48.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 693,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844810,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892055,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 235913,
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
          "id": "d908b58481ed170cadf58f949ce45cf3d2db52bb",
          "message": "Merge pull request #42 from kaidaniel/iter/free-39\n\nIteration 39: annotated parameters' fields and elements (dataclasses, pydantic), one-item tuples in hovers",
          "timestamp": "2026-10-17T07:24:57+01:00",
          "tree_id": "0b1d597ce06d3146afdb797756535986e3d340b8",
          "url": "https://github.com/kaidaniel/rowan/commit/d908b58481ed170cadf58f949ce45cf3d2db52bb"
        },
        "date": 1792218521252,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.72,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441601,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 477213,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103944,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33544,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 89,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311381,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23647,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 20.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 342,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 402565,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 431781,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87182,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99457,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 651,
            "unit": "cases",
            "extra": "645 passing"
          },
          {
            "name": "pip: wall time",
            "value": 41.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 267,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1062762,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1133350,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223873,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85378,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 26.58,
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
            "value": 506831,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531528,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86885,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 20.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 301,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436219,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512860,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107448,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408656,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421874,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64662,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30945,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 48.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 701,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844798,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892044,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 235964,
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
          "id": "82a5fb500ed6324a11219ba38b231c2c3ebff830",
          "message": "Merge pull request #43 from kaidaniel/iter/site-packages\n\nExperiment: following installed packages (site-packages) with a per-entry allowance",
          "timestamp": "2026-10-17T08:42:32+01:00",
          "tree_id": "035c479e010c9ed0570a888c49f8c4f4d8c2cc0b",
          "url": "https://github.com/kaidaniel/rowan/commit/82a5fb500ed6324a11219ba38b231c2c3ebff830"
        },
        "date": 1792223157449,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 441601,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 477213,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 103944,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33544,
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
            "value": 83,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283382,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311381,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23647,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 355,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 401307,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433898,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87608,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99246,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 652,
            "unit": "cases",
            "extra": "646 passing"
          },
          {
            "name": "pip: wall time",
            "value": 40.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 263,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1062762,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1133350,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 223873,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 85378,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 25.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 129,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 506831,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531528,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86885,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 34.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 291,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 436219,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 512860,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 107448,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 104202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 114,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 408656,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 421874,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 64662,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 30945,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 38.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 680,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844798,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892044,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169373,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 235964,
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
          "id": "bf7458b67e97aae76c65a41417ea1f3931a13d1e",
          "message": "Merge pull request #44 from kaidaniel/iter/backlog-41\n\nIteration 41: soundness and precision backlog",
          "timestamp": "2026-10-17T10:07:08+01:00",
          "tree_id": "5c258319ed49e91c7a5086b8bcbf548307035ab2",
          "url": "https://github.com/kaidaniel/rowan/commit/bf7458b67e97aae76c65a41417ea1f3931a13d1e"
        },
        "date": 1792228248489,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 119,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443273,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 479113,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104532,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33526,
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
            "value": 87,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283383,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311507,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23650,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 355,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400609,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 433325,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99391,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 655,
            "unit": "cases",
            "extra": "651 passing"
          },
          {
            "name": "pip: wall time",
            "value": 44.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 266,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1145889,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1222620,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241197,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87989,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 137,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 508043,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 533344,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 24111,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 37.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 307,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 469084,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 549453,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 113796,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 122037,
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
            "value": 134,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 463325,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 479947,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 77976,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 37222,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 49.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 687,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844804,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 892094,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 235996,
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
          "id": "a00f66566a59367512b4d93e6b841f0ffaad4f2f",
          "message": "Merge pull request #45 from kaidaniel/iter/free-42\n\nIteration 42: hovers per place of a name, __init__ on a new instance, joined expressions unnamed",
          "timestamp": "2026-10-17T11:02:41+01:00",
          "tree_id": "6852730592f8346667662ff4c784bdc761bb71d2",
          "url": "https://github.com/kaidaniel/rowan/commit/a00f66566a59367512b4d93e6b841f0ffaad4f2f"
        },
        "date": 1792231561906,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 16.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 478918,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33563,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 87,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283352,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311399,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48950,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23661,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 347,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400162,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 431573,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87431,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100082,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 655,
            "unit": "cases",
            "extra": "654 passing"
          },
          {
            "name": "pip: wall time",
            "value": 23.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 264,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1145705,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1221864,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241152,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87768,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 27.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 139,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 507978,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532925,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87098,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23819,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 37.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 307,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 468820,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 549191,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 113713,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 122052,
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
            "value": 133,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 463116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 479682,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 78038,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 37548,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 27.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 684,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 844434,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891711,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169329,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 235819,
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
          "id": "d88cd43977597a03ce4b1aff635fa7edbda814f8",
          "message": "Merge pull request #46 from kaidaniel/iter/backlog-43\n\nIteration 43: false warnings and cost left by #44 / #45, small soundness items",
          "timestamp": "2026-10-17T12:01:03+01:00",
          "tree_id": "34e5a2b3f0b54e99e41b45efc2219e94876c368a",
          "url": "https://github.com/kaidaniel/rowan/commit/d88cd43977597a03ce4b1aff635fa7edbda814f8"
        },
        "date": 1792235054092,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 478844,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33563,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283352,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311207,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48950,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23563,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 15.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 346,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400162,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 431573,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87431,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100081,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 655,
            "unit": "cases",
            "extra": "661 passing"
          },
          {
            "name": "pip: wall time",
            "value": 44.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 266,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1145705,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1221931,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241152,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87768,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 27.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 135,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 507978,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532925,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87098,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23819,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 36.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 305,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 468820,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 549191,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 113713,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 122052,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 116,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421151,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 434723,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70084,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31625,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 46.57,
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
            "value": 845129,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891479,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169428,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230777,
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
          "id": "767bc5b3ac08af02325fbbf577f6101055d6996e",
          "message": "Merge pull request #47 from kaidaniel/iter/editor-latency\n\nEditor latency: timm's 157 s hovers and sktime's 7.6 s hover fixed, timm's first hover 3.5 -> 2.3 s, closure walks resumed and exact",
          "timestamp": "2026-10-17T14:49:33+01:00",
          "tree_id": "64e8c1c52078e92ecb13ebdfddb1ceb26bb208a5",
          "url": "https://github.com/kaidaniel/rowan/commit/767bc5b3ac08af02325fbbf577f6101055d6996e"
        },
        "date": 1792245180623,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 478844,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33563,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 9.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283481,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311341,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48970,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23604,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 349,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400162,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 431573,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87431,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100081,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 656,
            "unit": "cases",
            "extra": "663 passing"
          },
          {
            "name": "pip: wall time",
            "value": 41.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1145529,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1217621,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241193,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87793,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 35.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 147,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 725300,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 746854,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 125831,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23839,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 35.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 308,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 468820,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 549191,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 113713,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 122052,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421151,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 434723,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70084,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31625,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 44.48,
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
            "value": 845129,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891851,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169428,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230279,
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
          "id": "736dc79ba0267aa2033bf1f23b51adf525d662da",
          "message": "Merge pull request #48 from kaidaniel/iter/free-45\n\nIteration 45: a parameter's Requirements in its body, typed values from builtins, quieter hovers",
          "timestamp": "2026-10-17T16:02:22+01:00",
          "tree_id": "ae251780762eec1cb0b1c69b94ab30bf36c23079",
          "url": "https://github.com/kaidaniel/rowan/commit/736dc79ba0267aa2033bf1f23b51adf525d662da"
        },
        "date": 1792249560305,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 12.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 119,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 478830,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
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
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 87,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283166,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311127,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48915,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 348,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 400162,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 431583,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 87431,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 100416,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 656,
            "unit": "cases",
            "extra": "667 passing"
          },
          {
            "name": "pip: wall time",
            "value": 23.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 275,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1144977,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1216411,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241202,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88081,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 38.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 148,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 725300,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 746848,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 125831,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 23937,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 33.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 307,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 468820,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 549191,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 113713,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 122060,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 11.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 434828,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31753,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 35.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 694,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 845129,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 891851,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 169428,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 230334,
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
          "id": "ca0b3dfd5d7d3da9b49bcdc747fb0daa150d974b",
          "message": "Merge pull request #49 from kaidaniel/iter/cost-46\n\nIteration 46: win back cost soundly",
          "timestamp": "2026-10-17T17:30:12+01:00",
          "tree_id": "58b3b1e75a22af77e08bd1e1311cb7f88431ac04",
          "url": "https://github.com/kaidaniel/rowan/commit/ca0b3dfd5d7d3da9b49bcdc747fb0daa150d974b"
        },
        "date": 1792254836416,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 121,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494391,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
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
            "value": 9.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283166,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312416,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48915,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23496,
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
            "value": 346,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 401162,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 473392,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 73941,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 97496,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 656,
            "unit": "cases",
            "extra": "671 passing"
          },
          {
            "name": "pip: wall time",
            "value": 34.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 278,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1141220,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1219477,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241006,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 86599,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 29.1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 132,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 503998,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521549,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87381,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22736,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 223,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275043,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 352612,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53894,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74319,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.28,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 117,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436852,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31753,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 235,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 32109,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 42545,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 4536,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 33750,
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
          "id": "1dfb4cf09745322869a9f81d1c429b50dad76ce1",
          "message": "Merge pull request #50 from kaidaniel/iter/soundness-47\n\nIteration 47: soundness of library calls on instances, % formatting, registry idioms, typed values",
          "timestamp": "2026-10-17T19:01:06+01:00",
          "tree_id": "651ab24b65067e416c327e01c14286f63b913d9b",
          "url": "https://github.com/kaidaniel/rowan/commit/1dfb4cf09745322869a9f81d1c429b50dad76ce1"
        },
        "date": 1792260284863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 119,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494373,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 13.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 354,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403011,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481210,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74471,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 659,
            "unit": "cases",
            "extra": "676 passing"
          },
          {
            "name": "pip: wall time",
            "value": 44.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 272,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148711,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1230789,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242513,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87015,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 24.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 131,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 503858,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521616,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87345,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22736,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 224,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353244,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74122,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436929,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 250,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66197,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 80476,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12291,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 40598,
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
          "id": "6c8fc31e2188129d1d61952d98abcb2aa5519290",
          "message": "Merge pull request #51 from kaidaniel/iter/free-48\n\nIteration 48: pandas Series hold their values (soundness + exact values for literal frames); x in xs",
          "timestamp": "2026-10-17T20:37:47+01:00",
          "tree_id": "7ee60dff0a5e45a11c9b92d7f0e65b75b0d3088d",
          "url": "https://github.com/kaidaniel/rowan/commit/6c8fc31e2188129d1d61952d98abcb2aa5519290"
        },
        "date": 1792266083562,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 117,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494373,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 87,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 350,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403011,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481210,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74471,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 659,
            "unit": "cases",
            "extra": "685 passing"
          },
          {
            "name": "pip: wall time",
            "value": 42.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 272,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148711,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1230789,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242513,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87015,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 23.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 132,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 503858,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521616,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87345,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22736,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 10.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 228,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353244,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74123,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 9.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436929,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 242,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66197,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 80476,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12291,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 40598,
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
          "id": "31463cc339796f6764b710baaa5bda8cc6a672d4",
          "message": "Merge pull request #52 from kaidaniel/iter/sitepkg-bench-49\n\nIteration 49: a site-packages load benchmark (bench/sitepackages.py)",
          "timestamp": "2026-10-17T21:26:27+01:00",
          "tree_id": "3ae099fda54b0e61e9c2e451a187bd5e31c88a0f",
          "url": "https://github.com/kaidaniel/rowan/commit/31463cc339796f6764b710baaa5bda8cc6a672d4"
        },
        "date": 1792268996189,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 13.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 117,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494373,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 21.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 348,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403011,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481210,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74471,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 659,
            "unit": "cases",
            "extra": "685 passing"
          },
          {
            "name": "pip: wall time",
            "value": 48.71,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 277,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148711,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1230789,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242513,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87015,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 23.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 132,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 503858,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521616,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87345,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22736,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 224,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353244,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74123,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436929,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 241,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66197,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 80476,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12291,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 40598,
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
          "id": "4da1b37a15712bd219b14b4bc789e2ff7c64dce0",
          "message": "Merge pull request #53 from kaidaniel/iter/dataclass-fields-50\n\nIteration 50: dataclass default factories, annotated field types, the list[x] legend",
          "timestamp": "2026-10-17T22:14:06+01:00",
          "tree_id": "c3d37a4df907411d2bf2f89d10f094daaff4fc1d",
          "url": "https://github.com/kaidaniel/rowan/commit/4da1b37a15712bd219b14b4bc789e2ff7c64dce0"
        },
        "date": 1792271844692,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 117,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494373,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 350,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403011,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74471,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 660,
            "unit": "cases",
            "extra": "690 passing"
          },
          {
            "name": "pip: wall time",
            "value": 41.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 272,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148594,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1230436,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242497,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 86945,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: wall time",
            "value": 19.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 137,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 504158,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521882,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87350,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22716,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 224,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353244,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74123,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 116,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436929,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 242,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66213,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 77981,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12292,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41074,
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
          "id": "1f6753e2fddee722e57ffec4bc1eba4bd64019d5",
          "message": "Merge pull request #54 from kaidaniel/iter/ci-workflows-51\n\nIteration 51: accumulated tests and benchmarks into CI",
          "timestamp": "2026-10-17T23:31:47+01:00",
          "tree_id": "8ed07f5f34237c405be88a585d1692e8d2ff955c",
          "url": "https://github.com/kaidaniel/rowan/commit/1f6753e2fddee722e57ffec4bc1eba4bd64019d5"
        },
        "date": 1792276673185,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494373,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33714,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 350,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 403011,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481135,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74471,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99144,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 660,
            "unit": "cases",
            "extra": "691 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10120,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 274,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10458,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 490.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1207.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 28.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 278,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148594,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1230436,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242497,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 86945,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1444,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 156,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 441,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 820.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1320.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 19.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 137,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 504158,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 521882,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87350,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22716,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39371,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 51,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.32,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26500,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 53,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.09,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 6935,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 35,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 33404,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 10522,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 0.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 40187,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 76,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 220,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353244,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74123,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421263,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436929,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31787,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 247,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66213,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 77981,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12292,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41074,
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
          "id": "099cffa1725705e0f37493444067fee61d3b5a4a",
          "message": "Merge pull request #55 from kaidaniel/iter/free-52\n\nFree iteration 52: soundness of keys that may be objects, class attributes on some paths",
          "timestamp": "2026-10-18T00:43:48+01:00",
          "tree_id": "256498989583cf763e699a754905ecac201db822",
          "url": "https://github.com/kaidaniel/rowan/commit/099cffa1725705e0f37493444067fee61d3b5a4a"
        },
        "date": 1792280993476,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 20.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 117,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 443248,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 494519,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 104522,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33758,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283295,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312605,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48935,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23567,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 352,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 402989,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 481113,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 74461,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99132,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 660,
            "unit": "cases",
            "extra": "698 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10121,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 297,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10777,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 498.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1188,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 33.99,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 272,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148316,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1245602,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 242448,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87168,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1417,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 147,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 470,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 744.1,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1888,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 23.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 504158,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 524966,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87350,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22752,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.74,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39371,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 24341,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 12,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 33404,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.21,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 10522,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 1.04,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 40187,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 76,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 219,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275454,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353243,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 53844,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 74100,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 116,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 421211,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 436877,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 70117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31528,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 242,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 66213,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 77981,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 12292,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41074,
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
          "id": "d5adf633a7f2f9dffca6aaaac63766f93ee99be9",
          "message": "Merge pull request #56 from kaidaniel/iter/framework-blowup-53\n\nIteration 53: why framework-like code blows up (recursion through contexts, merged receivers, join facts, import allowance)",
          "timestamp": "2026-10-18T03:03:21+01:00",
          "tree_id": "18b23d4e0d9289c135d21c2c837c8628e75be7ab",
          "url": "https://github.com/kaidaniel/rowan/commit/d5adf633a7f2f9dffca6aaaac63766f93ee99be9"
        },
        "date": 1792289357274,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 20.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 434356,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 483607,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 99304,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31901,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312563,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48940,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24178,
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
            "value": 294,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 402021,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 487836,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 73088,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 95433,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 662,
            "unit": "cases",
            "extra": "699 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10127,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 291,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10607,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 501.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1146.9,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 34.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 270,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1133073,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1224383,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 241801,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87849,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1521,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 159,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 315,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 850.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1697.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 24.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498450,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518612,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86367,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21044,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.81,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39371,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26830,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 33,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.95,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 32643,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 84,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 18569,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 15,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 1.03,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 41559,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 82,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 230,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276344,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 360899,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73686,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414300,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429991,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69283,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31601,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 244,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52715,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 61870,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9033,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 40965,
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
          "id": "518571cc0235eec4dc23889f022b6da32dbab5dd",
          "message": "Merge pull request #57 from kaidaniel/iter/framework-blowup-54\n\nIteration 54: framework blow-up, continued (rounds x receivers, __getattr__ fallback, inspect)",
          "timestamp": "2026-10-18T05:41:15+01:00",
          "tree_id": "b968a54915abd5e1777cb920d1479120d2f7d31d",
          "url": "https://github.com/kaidaniel/rowan/commit/518571cc0235eec4dc23889f022b6da32dbab5dd"
        },
        "date": 1792298838130,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431071,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 492870,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96917,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312563,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48940,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24178,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 306,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399190,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 485892,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72163,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 94179,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 663,
            "unit": "cases",
            "extra": "702 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10123,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 281,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10446,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 483.1,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1189.4,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 43.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 265,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1151255,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1246363,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248877,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87989,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1009,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1226,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 168,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 179,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 1.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 511.1,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 882.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 23.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498450,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518612,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86367,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21044,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.77,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26885,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.2,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 34,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.95,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 32135,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 84,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 18569,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 15,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 1.04,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 42431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 76,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 225,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276360,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 360138,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54110,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73878,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414300,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429991,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69283,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31601,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 247,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52881,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62036,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9067,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41000,
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
          "id": "267eee19ffbd2844b89df283d97a0d0c90bd1107",
          "message": "Merge pull request #58 from kaidaniel/iter/free-55\n\nFree iteration 55: files in hovers and diagnostics, catch-alls in callers, handler classes, typed dict keys",
          "timestamp": "2026-10-18T08:17:05+01:00",
          "tree_id": "7bb6a79e5b26ac1b05740482af91611343917798",
          "url": "https://github.com/kaidaniel/rowan/commit/267eee19ffbd2844b89df283d97a0d0c90bd1107"
        },
        "date": 1792308165935,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431619,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 493578,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96956,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31065,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312563,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48940,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24178,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 310,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399146,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 485741,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72159,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93601,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 664,
            "unit": "cases",
            "extra": "707 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1013,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10146,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 293,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10937,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.1,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 447.4,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1146.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 46.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 280,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1151268,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1246377,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248880,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88005,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1009,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1291,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 157,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 216,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 679.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1038.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 23.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498450,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518612,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86367,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21044,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.77,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26885,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.16,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 32040,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 84,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 18569,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 15,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 0.92,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 42431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 10.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 224,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276344,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 360121,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73701,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 115,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414300,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429991,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69283,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31601,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 243,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62044,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41028,
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
          "id": "dabc5f6f9ac52786031ee9c4794aaa431f77ad50",
          "message": "Merge pull request #59 from kaidaniel/iter/precision-56\n\nIteration 56: precision on the small programs (recency per call, click handler)",
          "timestamp": "2026-10-18T09:42:46+01:00",
          "tree_id": "213039f79c7be7e4e961fb24119764c7720a0cc2",
          "url": "https://github.com/kaidaniel/rowan/commit/dabc5f6f9ac52786031ee9c4794aaa431f77ad50"
        },
        "date": 1792313330427,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 432313,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 492338,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97087,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 30992,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 92,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 283205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 312302,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48940,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 24179,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 16.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 305,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399166,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 485750,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72155,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93612,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 666,
            "unit": "cases",
            "extra": "710 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10113,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 289,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10738,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 518.3,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1282.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 43.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 279,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1151241,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1245827,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248873,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88069,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1406,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 154,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 448,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 904.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1294,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 16.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 132,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498110,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 517972,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86320,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21044,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.72,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26885,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.2,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.96,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 32055,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 18569,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 15,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 1.03,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 42431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 72,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 8.33,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 234,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276344,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 360121,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73701,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 6.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 127,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414433,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 430108,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69278,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31636,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 243,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62072,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41047,
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
          "id": "6621798e279d94a5d966cb63da19d09035b81c51",
          "message": "Merge pull request #60 from kaidaniel/iter/soundness-57\n\nSoundness sweep with the fuzzer (iteration 57)",
          "timestamp": "2026-10-18T13:26:17+01:00",
          "tree_id": "29137b1bfefee481998a111d1224d120d9c9f8ea",
          "url": "https://github.com/kaidaniel/rowan/commit/6621798e279d94a5d966cb63da19d09035b81c51"
        },
        "date": 1792326691970,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 21.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 430467,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 488980,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96671,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31606,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 281831,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311332,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48811,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23256,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 305,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399198,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 485616,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72171,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93604,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 667,
            "unit": "cases",
            "extra": "724 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10117,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 281,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10460,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.1,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 392.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 995,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 45.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 279,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152093,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1247050,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248976,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88028,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1422,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 160,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 435,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 913.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1586.5,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 23.47,
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
            "value": 498412,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518461,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21047,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.79,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 39426,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 26885,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.19,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 31435,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 18737,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 15,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 0.92,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 42431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 74,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 27,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 228,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276347,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 360113,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73545,
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
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413290,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428958,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69005,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31721,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.42,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 243,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62070,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "9fdd433edebda8b1d6016455e5d667c914142ebe",
          "message": "Merge pull request #61 from kaidaniel/iter/lib-budgets-58\n\nIteration 58: per-function budgets for installed packages; rowan serve follows the editor's interpreter",
          "timestamp": "2026-10-18T15:41:39+01:00",
          "tree_id": "e607f0d159950b298fc19ac06a695f4a8afa5d60",
          "url": "https://github.com/kaidaniel/rowan/commit/9fdd433edebda8b1d6016455e5d667c914142ebe"
        },
        "date": 1792334773408,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 19.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 111,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 430471,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 488393,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96671,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31606,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 281846,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311347,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48815,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23256,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 13.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 300,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399198,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 483294,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72171,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93604,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 668,
            "unit": "cases",
            "extra": "724 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10111,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 287,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10397,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 0.8,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 308.4,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 807.8,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 24.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 281,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152093,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1247025,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248976,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88028,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1400,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 160,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 300,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 853.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1336.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 25.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 125,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498412,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518461,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21047,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59893,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 53,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27083,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.15,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 71670,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19473,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 3.72,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 33116,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 385,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 28,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 218,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276347,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353691,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73545,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 124,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413290,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 428747,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69005,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31721,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 243,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62070,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "3b9b31593496998ef451aff15d89a76c370304e1",
          "message": "Merge pull request #62 from kaidaniel/iter/free-59\n\nFree iteration 59: unknowns keep the literals they may be (8080 | ?)",
          "timestamp": "2026-10-18T17:06:05+01:00",
          "tree_id": "5dda37e9c68ad519edad11fed4f7893bf287b248",
          "url": "https://github.com/kaidaniel/rowan/commit/3b9b31593496998ef451aff15d89a76c370304e1"
        },
        "date": 1792339920157,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 13.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 430471,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 488393,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96671,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31607,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 91,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 281875,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 311376,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48824,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23284,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 10.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 300,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399198,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 483294,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72171,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93604,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 668,
            "unit": "cases",
            "extra": "725 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1013,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10133,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 289,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10507,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 492,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1255.6,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 43.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 284,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152044,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1247208,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248947,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88119,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1007,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1212,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 163,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 277,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 1.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 513.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 801.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 16.4,
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
            "value": 498412,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 518461,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21059,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59806,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 53,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27983,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.16,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 71693,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 101,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19473,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 3.78,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 33120,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 383,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 28,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 8.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 221,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276347,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 353698,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73546,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 119,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413785,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 429142,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 68929,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31586,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 242,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 62070,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "15afdb37e599f667b7cca0d5f4b0be2eff3b3728",
          "message": "Merge pull request #63 from kaidaniel/iter/lib-skips-60\n\nIteration 60: cheaper calls that do not run in followed packages; a closure-cache soundness fix",
          "timestamp": "2026-10-20T10:13:33+01:00",
          "tree_id": "b14f9c7cf9da37ce4989769c169cf8298e423bb2",
          "url": "https://github.com/kaidaniel/rowan/commit/15afdb37e599f667b7cca0d5f4b0be2eff3b3728"
        },
        "date": 1792487959468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.33,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 129,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 430471,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 525516,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96671,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31607,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 281854,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 317749,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48821,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23308,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399198,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 515115,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72171,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93604,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 668,
            "unit": "cases",
            "extra": "728 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10129,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 299,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10539,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 531.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1256,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 46.29,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 288,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152044,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1276752,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248947,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88119,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1352,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 155,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 332,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 723.5,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1456.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 24.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 140,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498412,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531796,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86374,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21059,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.33,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59806,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 53,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.68,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27983,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 53,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.19,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.04,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 71550,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 103,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19473,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 3.9,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 33120,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 409,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 28,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 227,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276347,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377476,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73546,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 140,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413785,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 433599,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 68929,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31586,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.73,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 247,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63355,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "9b84695a74f639c923961cfcadc145bc82d5669a",
          "message": "Merge pull request #64 from kaidaniel/iter/deferred-defs-61\n\nIteration 61: inert decorators run past an entry's allowance; deferred definitions; four soundness fixes",
          "timestamp": "2026-10-20T11:57:54+01:00",
          "tree_id": "7637e7b7b634a7c0751ded32efaf028006af2c15",
          "url": "https://github.com/kaidaniel/rowan/commit/9b84695a74f639c923961cfcadc145bc82d5669a"
        },
        "date": 1792494242212,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 131,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 430471,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 525519,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 96671,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 31608,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.83,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 281854,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 317749,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 48821,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 23308,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.21,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 317,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399215,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517475,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72172,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93648,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 670,
            "unit": "cases",
            "extra": "732 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10148,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 285,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10490,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 521.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1282,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 25.81,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 295,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152092,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1276939,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248945,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88076,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1283,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 156,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 212,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.5,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 692,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1038.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 24.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 141,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498771,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 532346,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86421,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21060,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59806,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.73,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27983,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.18,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 70174,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 99,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19583,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 10.26,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 457,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 38.94,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 689,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 6,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 4.7,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 98225,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 246,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 11,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 10.21,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 194,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 2,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 14079,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 126,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 236,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276347,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377476,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73546,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.63,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 127,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 413773,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 435811,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 68922,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 31679,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 248,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63355,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "7b0af39ca813b2fd542b2b9269fdf131d31b8564",
          "message": "Merge pull request #65 from kaidaniel/iter/free-62\n\nTyped unknowns kept: None | int(?) and builtins' dunder results (free iteration 62)",
          "timestamp": "2026-10-20T12:59:39+01:00",
          "tree_id": "e69847e30da24f9fe117650ea17215a7e4a2dd20",
          "url": "https://github.com/kaidaniel/rowan/commit/7b0af39ca813b2fd542b2b9269fdf131d31b8564"
        },
        "date": 1792497969072,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 145,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431688,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518357,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97623,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 32589,
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
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 277373,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 313056,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46617,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22182,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.11,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 319,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399149,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517658,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72184,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93582,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 670,
            "unit": "cases",
            "extra": "735 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10107,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 279,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10462,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 545.6,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1256,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 32.93,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 298,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1152987,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1278046,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 249754,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88426,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1366,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 158,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 441,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 4.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 767.1,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1297.5,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 12.38,
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
            "value": 497793,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531139,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20946,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.26,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.77,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.18,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 32,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.22,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 69857,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 101,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19557,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 10.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 468,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 36.18,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 688,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 6,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 4.78,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 98225,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 251,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 11,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 10.09,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 196,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 2,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 14079,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 128,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 9.89,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 248,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 276244,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377335,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54106,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73469,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 154,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414479,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 446390,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69807,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 33953,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.02,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 254,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63355,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "5a163006855baca409ef7a0643266f0aa7de2b15",
          "message": "Merge pull request #66 from kaidaniel/iter/follow-never-worse-63\n\nIteration 63: following installed packages is never worse than not following",
          "timestamp": "2026-10-20T15:55:18+01:00",
          "tree_id": "2f4b06bb37b33bd11d026c7c447f649eb7176155",
          "url": "https://github.com/kaidaniel/rowan/commit/5a163006855baca409ef7a0643266f0aa7de2b15"
        },
        "date": 1792508543356,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 15.05,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 140,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431688,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518412,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97623,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 32589,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.83,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307902,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 14.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 317,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399149,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517684,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72184,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93596,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 671,
            "unit": "cases",
            "extra": "739 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10126,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 299,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10509,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 527.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1285.3,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 27.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 293,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1150373,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1270560,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247517,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1376,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 158,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 325,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 763.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1526.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 21.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 140,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497793,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 530553,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20919,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.28,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 2.11,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 69857,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 99,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 19557,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.42,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 60979,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 438,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 24,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 5.11,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 15456,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 313,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 24,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 104542,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 258,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.32,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 101,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 1.13,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 34585,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 138,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 234,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275850,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377733,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54063,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73501,
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
            "value": 155,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414473,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 446247,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69801,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 33929,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 6.08,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 245,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63366,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "ef3b8c02589434459a37ef85a850a6fd2259d6d8",
          "message": "Merge pull request #67 from kaidaniel/iter/lib-closure-64\n\nIteration 64: library calls that do not run share one closure",
          "timestamp": "2026-10-20T17:19:10+01:00",
          "tree_id": "c850ef59a8f7ea35f61c4b77f4d912497b9ba430",
          "url": "https://github.com/kaidaniel/rowan/commit/ef3b8c02589434459a37ef85a850a6fd2259d6d8"
        },
        "date": 1792513533143,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 140,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431688,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518412,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97623,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 32589,
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
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307902,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 314,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399149,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517684,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72184,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93596,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 671,
            "unit": "cases",
            "extra": "741 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1013,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10109,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 289,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10703,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 535.4,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1200.1,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 37.14,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 295,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1150373,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1270560,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247517,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88117,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1378,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 148,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 279,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 830.1,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1748,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 23.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 141,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497793,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 530553,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86379,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20919,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.75,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 56,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.09,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.08,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 61891,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.36,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 21250,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.31,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 286986,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 412,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 3.89,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 127053,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 429,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 2.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 116597,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 191,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 0.98,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 101,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.8,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 233,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275850,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377733,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54063,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73501,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 12.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 152,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 414473,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 446247,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 69801,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 33929,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.27,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 244,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63366,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "5853e65fb30f9b2fecd8e8ecbf48c32cd2d85303",
          "message": "Merge pull request #68 from kaidaniel/iter/free-65\n\nIteration 65 (free): result summaries for calls that do not run",
          "timestamp": "2026-10-20T19:11:37+01:00",
          "tree_id": "ac0349e26771b88beb6c5e4a9474ccf4eb128302",
          "url": "https://github.com/kaidaniel/rowan/commit/5853e65fb30f9b2fecd8e8ecbf48c32cd2d85303"
        },
        "date": 1792520298890,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.65,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 161,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431675,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518831,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97623,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 32555,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.95,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307902,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
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
            "value": 420,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 398945,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517890,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72137,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93642,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 653,
            "unit": "cases",
            "extra": "760 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10108,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 345,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10435,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 511.4,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1177.6,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 39.62,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 360,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1150727,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1270731,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247607,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88135,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1395,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 169,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 700,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 945.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1631.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 24.57,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 158,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497636,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 531795,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86439,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 20909,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.16,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.15,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 61891,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 21250,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 216872,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 392,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 127089,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 424,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 3.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 116597,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 191,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.33,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 101,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.71,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 9.08,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 292,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275833,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377689,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54081,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73499,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.99,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 167,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 426980,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 459779,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 72126,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36894,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 5.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 340,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52884,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63309,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "d3f7aeee99aa678b0420dc82bfb9e559ccb1133f",
          "message": "Merge pull request #69 from kaidaniel/iter/summary-costs-66\n\nIteration 66: the costs of result summaries (#68 follow-up)",
          "timestamp": "2026-10-20T20:57:23+01:00",
          "tree_id": "f60dfabc59c47fbaf46becb71e8ef5797e90b8a4",
          "url": "https://github.com/kaidaniel/rowan/commit/d3f7aeee99aa678b0420dc82bfb9e559ccb1133f"
        },
        "date": 1792526608303,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 148,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 431688,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518412,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 97623,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 32589,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307902,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 10.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 318,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399131,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 517723,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72178,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93599,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 653,
            "unit": "cases",
            "extra": "762 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10103,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 285,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10597,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 556.7,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1204,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 33.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 352,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1151842,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1272070,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247808,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88095,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1427,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 168,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 352,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 960.4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1443.3,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 22.11,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 148,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 497125,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 530136,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86179,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21538,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.76,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 56,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.38,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 54,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.09,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 8,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.06,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 61670,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 92,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.36,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 21250,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 108,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.29,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 287525,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 415,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 3.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 127053,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 427,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 2.31,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 116597,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 194,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 0.92,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 113,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32521,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 234,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 275850,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 377733,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54063,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 73501,
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
            "value": 167,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 426980,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 459779,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 72126,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36894,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 2.47,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 245,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63214,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "180b0926a663aa063a608dca6d6d10946c4ab56a",
          "message": "Merge pull request #70 from kaidaniel/iter/soundness-67\n\nSoundness of iterations 60-66's mechanisms (iteration 67)",
          "timestamp": "2026-10-21T00:07:04+01:00",
          "tree_id": "f97bbdfd57224a8eba8704f9981bad36669a6ed3",
          "url": "https://github.com/kaidaniel/rowan/commit/180b0926a663aa063a608dca6d6d10946c4ab56a"
        },
        "date": 1792538010919,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 18.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 143,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427039,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 516790,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98285,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33644,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 12.15,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 94,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307810,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 14.9,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 309,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399233,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 518708,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72178,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 93653,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 654,
            "unit": "cases",
            "extra": "773 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10107,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 269,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10447,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 490.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1188.3,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 46.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 340,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1154882,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1277298,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248651,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87822,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1009,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1363,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 176,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 240,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 674.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1090.2,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 19.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 141,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 496413,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 529553,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86220,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21175,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59719,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 51,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27972,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 50,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 52192,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 88,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 21020,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 172394,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 376,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 5.29,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 125511,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 415,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 3.84,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 115553,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 188,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.26,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6323,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.79,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32500,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 211,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289887,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 387348,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54942,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 80936,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 171,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 426980,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 459866,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 72126,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36894,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.27,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 233,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63215,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "58a09e04d98e59f0a9a5a17c3a763f953d594536",
          "message": "Merge pull request #71 from kaidaniel/iter/free-68\n\niter/free-68: decorators and generators run on some paths; one-shot builtin iterators",
          "timestamp": "2026-10-21T01:30:06+01:00",
          "tree_id": "7094988737133f3351460c596653f01623e67eb4",
          "url": "https://github.com/kaidaniel/rowan/commit/58a09e04d98e59f0a9a5a17c3a763f953d594536"
        },
        "date": 1792542876749,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 14.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 144,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428147,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 518764,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98281,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33692,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 8.48,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307810,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 17.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 312,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 399140,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 518638,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 72182,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 92500,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 657,
            "unit": "cases",
            "extra": "776 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10120,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 273,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10422,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 0.8,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 274.7,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 677.3,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 36.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 323,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148313,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1269674,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 246583,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 86915,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1499,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 158,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 528,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 913.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1296.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 20.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 139,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 494241,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 528755,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 85300,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21185,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.9,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 59707,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 27960,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 50,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 0.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 52289,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.35,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20955,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 296274,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 414,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 3.77,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 127239,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 414,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 2.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 115561,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 191,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 0.84,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6399,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 100,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32500,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 112,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 210,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289423,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 386248,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54878,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81051,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 12.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 155,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 429490,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 463243,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 73932,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36592,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 2.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 238,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63218,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "148fd92b606295c2e15a0774bf079da301f845dc",
          "message": "Merge pull request #72 from kaidaniel/iter/soundness-69\n\nIteration 69: three known unsound claims (live iterators, lru_cache wrappers, skipped calls returning functions)",
          "timestamp": "2026-10-21T04:53:00+01:00",
          "tree_id": "ff094a455bd47040c1b19492047f8289047df206",
          "url": "https://github.com/kaidaniel/rowan/commit/148fd92b606295c2e15a0774bf079da301f845dc"
        },
        "date": 1792555162028,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 23.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 142,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428505,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 519398,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98652,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33717,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.93,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307814,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 45739,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22181,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 12.14,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 330,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 410049,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 537059,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 75206,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 97679,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 658,
            "unit": "cases",
            "extra": "782 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10099,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 270,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10444,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 494.3,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1110.6,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 39.97,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 335,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1147736,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1269600,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 246560,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87053,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1487,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 158,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 390,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 919.4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1978.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 15.25,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 145,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 495946,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 530728,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 85727,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 21209,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.91,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60177,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28430,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 50,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.14,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 52289,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20953,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.25,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 258407,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 392,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 4.36,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 126991,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 415,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 2.92,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 114707,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 190,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 0.99,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6334,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.56,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32850,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 118,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 212,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289425,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 386257,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54878,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81096,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 156,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 429490,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 463248,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 73932,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36606,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 231,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63220,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41044,
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
          "id": "d21c985d7ee1242bc47687955c8298615f2c9595",
          "message": "Merge pull request #73 from kaidaniel/iter/free-70\n\nIteration 70: lazy generator expressions (and two generator/zip soundness fixes)",
          "timestamp": "2026-10-21T07:06:00+01:00",
          "tree_id": "9b0007f12e941118d0f6c9a44e2e55305555b66a",
          "url": "https://github.com/kaidaniel/rowan/commit/d21c985d7ee1242bc47687955c8298615f2c9595"
        },
        "date": 1792563097398,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 19.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 141,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428466,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 519366,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98658,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33732,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 7.18,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 308101,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46091,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22216,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 341,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 410304,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 541157,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 76057,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98701,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 659,
            "unit": "cases",
            "extra": "784 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10123,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 281,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10401,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 0.9,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 318.9,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 703.7,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 50.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 350,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148394,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1276275,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 250165,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 89080,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1009,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1327,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 170,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 233,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.6,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 598.4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1094.4,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 18.01,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 145,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 496261,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 533309,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86655,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22044,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60177,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 51,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.69,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28430,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 50,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.18,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 36,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 51297,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 86,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.64,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20953,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.32,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 170736,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 369,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 121572,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 411,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 4.16,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 114469,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 188,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.34,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6334,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.78,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32735,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 8.03,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 216,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289425,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 386850,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54887,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81137,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 13.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 156,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 430721,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 464538,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 74531,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36686,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 2.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 244,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63221,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41045,
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
          "id": "6250fe8eb6d56a1c23f83b10ee6bbb052362f45d",
          "message": "Merge pull request #74 from kaidaniel/iter/shortcircuit-71\n\nIteration 71: any/all/next/in stop early; click parser.py:400 traced; cost",
          "timestamp": "2026-10-21T08:38:48+01:00",
          "tree_id": "8960469a9ef0fb348e64f7703e63561da690edb9",
          "url": "https://github.com/kaidaniel/rowan/commit/6250fe8eb6d56a1c23f83b10ee6bbb052362f45d"
        },
        "date": 1792568685424,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 22.66,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 142,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428466,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 519586,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98658,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33734,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 11.14,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276495,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 308081,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46091,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 18.51,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 334,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 410238,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 541025,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 76035,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98917,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 663,
            "unit": "cases",
            "extra": "786 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10111,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 283,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10964,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 508,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1151.8,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 49.55,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 351,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148936,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1276491,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 248786,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88812,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1008,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1247,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 187,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 235,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 1.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 365.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 598.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 17.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 143,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 496261,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 533429,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86655,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22048,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.94,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60177,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 64,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.52,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28430,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 34,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.04,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 50118,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 84,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.42,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20953,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.29,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 239313,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 384,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 4.71,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 126938,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 417,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 3.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 113205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 197,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6334,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 102,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32735,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.24,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 215,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289497,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 387967,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54905,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81231,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 10.94,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 154,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 430721,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 464566,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 74531,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36688,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.17,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 236,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63223,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41055,
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
          "id": "eb88e6f1f4f4d2b7f6049b4996c69efcb042efdb",
          "message": "Merge pull request #75 from kaidaniel/iter/genclose-72\n\nIteration 72: generators closed with what holds them; raise-state cost",
          "timestamp": "2026-10-21T10:55:32+01:00",
          "tree_id": "1da001de2c17c1785a61e23e8f4a21affed01759",
          "url": "https://github.com/kaidaniel/rowan/commit/eb88e6f1f4f4d2b7f6049b4996c69efcb042efdb"
        },
        "date": 1792576823836,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 24.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 139,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428466,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 519586,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98658,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33734,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 6.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276729,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 308305,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46112,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.21,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 343,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 410418,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 541234,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 76050,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98916,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 668,
            "unit": "cases",
            "extra": "787 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1011,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10098,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 279,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10417,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 0.8,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 290,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 735.9,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 36.6,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 349,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1144958,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1272583,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247657,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87514,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1369,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 169,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 266,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 756.7,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 981.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 13.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 146,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 498148,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 539457,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 86928,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22118,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.95,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60178,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 62,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 34,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.07,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 51526,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 84,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.44,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20953,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.28,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 246000,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 390,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 4.41,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 126908,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 416,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 3.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 113205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 195,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1.01,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6334,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 104,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.58,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32735,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 120,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 7.43,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 214,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289497,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 387979,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54905,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81231,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.13,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 154,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 430721,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 464554,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 74531,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36688,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.02,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 233,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63223,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41055,
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
          "id": "cf973f4f4ea7e0c4b210f502d4dc8478bcb6ff8c",
          "message": "Merge pull request #76 from kaidaniel/iter/free-73\n\nIteration 73: generator soundness in loops (per-item effects, raises where asked)",
          "timestamp": "2026-10-21T13:20:18+01:00",
          "tree_id": "d9f69d8452b0b2dc29e93174dc57f98c0422cc79",
          "url": "https://github.com/kaidaniel/rowan/commit/cf973f4f4ea7e0c4b210f502d4dc8478bcb6ff8c"
        },
        "date": 1792585610336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "click: wall time",
            "value": 17.45,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "click: peak memory",
            "value": 144,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 428466,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 520381,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98658,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33734,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 9.59,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 98,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276805,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 310167,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46152,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 19.16,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 341,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 410151,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 541072,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 76026,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 98916,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 672,
            "unit": "cases",
            "extra": "787 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10095,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 277,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10442,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 491.5,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1292.6,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 36.19,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 347,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1145770,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1299263,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 246846,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 87192,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1396,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 177,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 430,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 3.1,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 854,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1152.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 24.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 171,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 499444,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 548185,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 87124,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22104,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 0.79,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60178,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 64,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.39,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28431,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 34,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 56930,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 92,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.42,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 21040,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.32,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 290138,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 415,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 3.61,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 126908,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 435,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 2.37,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 113205,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 197,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 0.94,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 6334,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 103,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.49,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32710,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 11.54,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 214,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289497,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 387995,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54905,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81231,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 8.08,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 161,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 430721,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 464597,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 74531,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36688,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 4.13,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 232,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52732,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63223,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9070,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41055,
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
          "id": "ee7d8215dbd6d88878d20571e538aa91d84493ca",
          "message": "Merge pull request #78 from kaidaniel/iter/free-75\n\nFailing reads raise to finally blocks; one finally run for raises leaving the program",
          "timestamp": "2026-10-21T17:03:19+01:00",
          "tree_id": "08af540c4aaa4f676546dc7fb452c5f383dabb68",
          "url": "https://github.com/kaidaniel/rowan/commit/ee7d8215dbd6d88878d20571e538aa91d84493ca"
        },
        "date": 1792599058073,
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
            "value": 142,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "click: work",
            "value": 427886,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "click: work units",
            "value": 519587,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "click: tabulations",
            "value": 98463,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "click: objects",
            "value": 33733,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: wall time",
            "value": 10.24,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "email: peak memory",
            "value": 96,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "email: work",
            "value": 276805,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "email: work units",
            "value": 307389,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "email: tabulations",
            "value": 46152,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "email: objects",
            "value": 22217,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: wall time",
            "value": 11.89,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "functional: peak memory",
            "value": 341,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "functional: work",
            "value": 412704,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "functional: work units",
            "value": 543518,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "functional: tabulations",
            "value": 76435,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "functional: objects",
            "value": 99028,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "golden tab=0: failing cases",
            "value": 677,
            "unit": "cases",
            "extra": "789 passing"
          },
          {
            "name": "pip first hover: p50",
            "value": 1012,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: max",
            "value": 10092,
            "unit": "ms",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip first hover: peak memory",
            "value": 289,
            "unit": "MB",
            "extra": "12 modules, 6 on demand"
          },
          {
            "name": "pip hovers: setup",
            "value": 10411,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p50",
            "value": 1.2,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: p99",
            "value": 510.9,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip hovers: max",
            "value": 1458,
            "unit": "ms",
            "extra": "4828 functions on demand"
          },
          {
            "name": "pip: wall time",
            "value": 45.23,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "pip: peak memory",
            "value": 344,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "pip: work",
            "value": 1148365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "pip: work units",
            "value": 1292110,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "pip: tabulations",
            "value": 247246,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "pip: objects",
            "value": 88243,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich first hover: p50",
            "value": 1010,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: max",
            "value": 1353,
            "unit": "ms",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich first hover: peak memory",
            "value": 189,
            "unit": "MB",
            "extra": "12 modules, 8 on demand"
          },
          {
            "name": "rich hovers: setup",
            "value": 443,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p50",
            "value": 2.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: p99",
            "value": 822.9,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich hovers: max",
            "value": 1019.8,
            "unit": "ms",
            "extra": "832 functions on demand"
          },
          {
            "name": "rich: wall time",
            "value": 16.21,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "rich: peak memory",
            "value": 172,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "rich: work",
            "value": 489034,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "rich: work units",
            "value": 533290,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "rich: tabulations",
            "value": 85722,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "rich: objects",
            "value": 22098,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: wall time",
            "value": 1.03,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: work",
            "value": 60178,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: peak memory",
            "value": 62,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s1_small_deps follow: unknown hovers",
            "value": 9,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: wall time",
            "value": 0.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: work",
            "value": 28457,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: peak memory",
            "value": 52,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: wall time",
            "value": 0.15,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: work",
            "value": 7337,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: peak memory",
            "value": 34,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s2_internal only: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: wall time",
            "value": 1.46,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: work",
            "value": 59365,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: peak memory",
            "value": 90,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s3_cli follow: unknown hovers",
            "value": 14,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: wall time",
            "value": 0.53,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: work",
            "value": 20592,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: peak memory",
            "value": 106,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s4_web follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: wall time",
            "value": 5.29,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: work",
            "value": 242335,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: peak memory",
            "value": 387,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s5_data follow: unknown hovers",
            "value": 21,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: wall time",
            "value": 5.3,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: work",
            "value": 121558,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: peak memory",
            "value": 425,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s6_symbolic follow: unknown hovers",
            "value": 16,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: wall time",
            "value": 3.36,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: work",
            "value": 110448,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: peak memory",
            "value": 195,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s7_orm follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: wall time",
            "value": 1,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: work",
            "value": 8311,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: peak memory",
            "value": 97,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s8_schema follow: unknown hovers",
            "value": 10,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: wall time",
            "value": 0.67,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: work",
            "value": 32555,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: peak memory",
            "value": 122,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "site-packages s9_graph follow: unknown hovers",
            "value": 7,
            "unit": "facts",
            "extra": "finished"
          },
          {
            "name": "sktime: wall time",
            "value": 12.5,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "sktime: peak memory",
            "value": 215,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "sktime: work",
            "value": 289487,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "sktime: work units",
            "value": 387986,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "sktime: tabulations",
            "value": 54895,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "sktime: objects",
            "value": 81231,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: wall time",
            "value": 13.4,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: peak memory",
            "value": 165,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work",
            "value": 441577,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: work units",
            "value": 474228,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: tabulations",
            "value": 75776,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "stdlib12k: objects",
            "value": 36842,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: wall time",
            "value": 3.12,
            "unit": "s",
            "extra": "finished"
          },
          {
            "name": "timm: peak memory",
            "value": 234,
            "unit": "MB",
            "extra": "finished"
          },
          {
            "name": "timm: work",
            "value": 52636,
            "unit": "statements",
            "extra": "finished"
          },
          {
            "name": "timm: work units",
            "value": 63000,
            "unit": "units",
            "extra": "finished"
          },
          {
            "name": "timm: tabulations",
            "value": 9054,
            "unit": "count",
            "extra": "finished"
          },
          {
            "name": "timm: objects",
            "value": 41056,
            "unit": "count",
            "extra": "finished"
          }
        ]
      }
    ]
  }
}