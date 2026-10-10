window.BENCHMARK_DATA = {
  "lastUpdate": 1791667210891,
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
      }
    ]
  }
}