window.BENCHMARK_DATA = {
  "lastUpdate": 1791709563046,
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
      }
    ]
  }
}