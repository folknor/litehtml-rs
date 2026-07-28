#!/usr/bin/env python3
"""Analyze y-drift between a Chrome reference dump and a pipeline dump.

Joins the two JSON dumps by dom path and reports:
1. "Drift sources": elements whose height differs from Chrome and whose
   matched children don't explain the difference (i.e. the mismatch
   originates there, typically a text leaf with rounded line-height).
2. The cumulative y-drift profile down the document.

Usage:
  python3 scripts/drift_analysis.py fixtures/<id>/chrome.json fixtures/<id>/<name>_pipeline.json
"""

import json
import sys
from collections import defaultdict


def load(path):
    with open(path) as f:
        return {el["path"]: el for el in json.load(f)}


def main():
    chrome = load(sys.argv[1])
    ours = load(sys.argv[2])

    # Detail mode: print full records for paths containing the given substring
    if len(sys.argv) > 3:
        needle = sys.argv[3]
        for p in sorted(set(chrome) | set(ours)):
            if needle in p:
                print(f"--- {p}")
                print(f"  chrome: {json.dumps(chrome.get(p))}")
                print(f"  ours:   {json.dumps(ours.get(p))}")
        return

    common = sorted(set(chrome) & set(ours), key=lambda p: chrome[p]["y"])
    print(f"chrome: {len(chrome)} elements, ours: {len(ours)}, matched by path: {len(common)}")

    chrome_only = sorted(set(chrome) - set(ours))
    ours_only = sorted(set(ours) - set(chrome))
    if chrome_only:
        print(f"\n=== {len(chrome_only)} chrome-only paths ===")
        for p in chrome_only[:20]:
            print(f"  {p}")
    if ours_only:
        print(f"\n=== {len(ours_only)} ours-only paths ===")
        for p in ours_only[:20]:
            print(f"  {p}")

    # Children index over matched paths
    children = defaultdict(list)
    for p in common:
        if ">" in p:
            parent = p.rsplit(">", 1)[0]
            children[parent].append(p)

    # Height mismatches not explained by matched children's mismatches
    sources = []
    for p in common:
        dh = ours[p]["h"] - chrome[p]["h"]
        if abs(dh) <= 0.5:
            continue
        child_dh = sum(ours[c]["h"] - chrome[c]["h"] for c in children[p])
        unexplained = dh - child_dh
        if abs(unexplained) > 0.5:
            sources.append((chrome[p]["y"], p, dh, unexplained))

    print(f"\n=== {len(sources)} drift-source elements (height mismatch not from children) ===")
    for y, p, dh, unexplained in sources[:40]:
        el = ours[p]
        fs = el.get("fontSize", "?")
        print(f"  y={y:8.1f} dh={dh:+6.1f} (own {unexplained:+6.1f}) fs={fs} "
              f"{el['tag']} h_ours={el['h']} h_chrome={chrome[p]['h']} {p[-100:]}")
    if len(sources) > 40:
        print(f"  ... and {len(sources) - 40} more")

    # Cumulative drift profile: dy at each 1000px band of the document
    print("\n=== drift profile (dy = ours.y - chrome.y, per 1000px band) ===")
    bands = defaultdict(list)
    for p in common:
        bands[int(chrome[p]["y"] // 1000)].append(ours[p]["y"] - chrome[p]["y"])
    for band in sorted(bands):
        dys = bands[band]
        print(f"  {band * 1000:6d}px: n={len(dys):4d} min={min(dys):+7.1f} "
              f"max={max(dys):+7.1f} mean={sum(dys) / len(dys):+7.1f}")


if __name__ == "__main__":
    main()
