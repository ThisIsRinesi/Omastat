#!/usr/bin/env python3
"""Compare release binaries against one read-only database or synthetic telemetry.

Example: python3 packaging/dev/benchmark-insights.py OLD NEW --database SNAPSHOT
Add --synthetic-intervals 200000 to use the snapshot's schema with generated data.
No telemetry from the source database is modified or copied into synthetic data.
"""
import argparse
import datetime as dt
import json
from pathlib import Path
import sqlite3
import statistics
import subprocess
import tempfile
import time


def synthetic(source, destination, count):
    original = sqlite3.connect(Path(source).resolve().as_uri() + "?mode=ro", uri=True)
    target = sqlite3.connect(destination)
    for (sql,) in original.execute(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    ):
        target.execute(sql)
    migrations = original.execute("SELECT * FROM schema_migrations").fetchall()
    placeholders = ",".join("?" for _ in migrations[0])
    target.executemany(f"INSERT INTO schema_migrations VALUES ({placeholders})", migrations)
    end = dt.datetime.combine(dt.date.today(), dt.time()).timestamp()
    start = (dt.datetime.combine(dt.date.today(), dt.time()) - dt.timedelta(days=56)).timestamp()
    start, end = int(start), int(end)
    target.execute(
        "INSERT INTO daemon_runs(started_at,last_heartbeat_at,stopped_at,stop_kind) VALUES(?,?,?,'clean')",
        (start, end, end),
    )
    step = max(2, (end-start)//count)
    for kind in ("focused", "open"):
        target.executemany(
            "INSERT INTO intervals(kind,app_class,started_at,ended_at,workspace) VALUES(?,?,?,?,?)",
            ((kind, f"app-{i % 12}", start+i*step, min(start+(i+1)*step,end), str(i % 4))
             for i in range(count) if start+i*step < end),
        )
    for (sql,) in original.execute(
        "SELECT sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL"
    ):
        target.execute(sql)
    target.commit()
    original.close()
    target.close()


def measure(binary, database, lens):
    start = time.perf_counter()
    result = subprocess.run(
        [str(Path(binary).resolve()), "--database", str(database), "insights", "--lens", lens, "--json"],
        capture_output=True, text=True, check=True,
    )
    return time.perf_counter()-start, json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--database", required=True)
    parser.add_argument("--synthetic-intervals", type=int, default=0)
    parser.add_argument("--runs", type=int, default=7)
    args = parser.parse_args()
    if args.runs < 1 or args.synthetic_intervals < 0:
        parser.error("runs must be positive and synthetic-intervals nonnegative")
    with tempfile.TemporaryDirectory(prefix="omastat-benchmark-") as directory:
        database = Path(args.database).resolve()
        if args.synthetic_intervals:
            database = Path(directory)/"synthetic.db"
            synthetic(args.database, database, args.synthetic_intervals)
        output = {}
        for lens in ("day", "week", "month", "life"):
            samples = {"before": [], "after": []}
            reports = {}
            # Warm both binaries, then alternate ordering to reduce cache/order effects.
            measure(args.before, database, lens)
            measure(args.after, database, lens)
            for run in range(args.runs):
                order = [("before", args.before), ("after", args.after)]
                if run % 2:
                    order.reverse()
                for name, binary in order:
                    elapsed, report = measure(binary, database, lens)
                    samples[name].append(elapsed)
                    reports[name] = report
            for key in ("focused_seconds", "open_seconds", "idle_seconds", "locked_seconds", "sleep_seconds"):
                assert key in reports["before"]["totals"], key
                assert reports["before"]["totals"][key] == reports["after"]["totals"][key], (lens,key)
            before, after = (statistics.median(samples[name])*1000 for name in ("before", "after"))
            output[lens] = {"before_ms": round(before,2), "after_ms": round(after,2),
                            "improvement_percent": round((1-after/before)*100,1)}
        print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
