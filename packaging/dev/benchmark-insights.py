#!/usr/bin/env python3
"""Compare release binaries against one read-only database or synthetic telemetry.

Example: python3 packaging/dev/benchmark-insights.py OLD NEW --database SNAPSHOT
Add --synthetic-intervals 200000 to use the snapshot's schema with generated data.
No telemetry from the source database is modified or copied into synthetic data.
"""
import argparse
import datetime as dt
import json
import math
from pathlib import Path
import sqlite3
import statistics
import sys
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
    end = time.time() - 60
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
            ((kind, ("chromium" if i % 3 == 0 else f"app-{i % 12}"), start+i*step, min(start+(i+1)*step,end), str(i % 4))
             for i in range(count) if start+i*step < end),
        )
    # Browser-heavy attribution with overlapping sources and changing domains.
    for i in range(0, count, 3):
        a, b = start+i*step, min(start+(i+1)*step, end)
        if a >= end:
            break
        for source, delay in [("browser", 0), ("overlap", step//3)]:
            target.execute(
                "INSERT INTO browser_domain_intervals(source,app_class,domain,started_at,ended_at,last_confirmed_at) VALUES(?,'chromium',?,?,?,?)",
                (source, f"site-{i % 17}.test", a+delay, b, b),
            )
    # Exercise multitasking as well as foreground attribution on current schemas.
    if target.execute("SELECT 1 FROM sqlite_master WHERE name='media_intervals'").fetchone():
        for i in range(0, count, 3):
            a, b = start+i*step, min(start+(i+3)*step, end)
            if a >= end:
                break
            for source, label in [("system", ""), ("browser:chromium", f"site-{i % 17}.test")]:
                target.execute(
                    "INSERT INTO media_intervals(source,app_class,label,started_at,ended_at,last_confirmed_at,ttl) VALUES(?,'chromium',?,?,?,?,90)",
                    (source, label, a, b, b),
                )
    for (sql,) in original.execute(
        "SELECT sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL"
    ):
        target.execute(sql)
    target.commit()
    original.close()
    target.close()


def measure(binary, database, lens, command, config, app, domain, offset=0):
    argv = [str(Path(binary).resolve()), "--database", str(database)]
    if config:
        argv += ["--config", config]
    argv += [command.split(":")[0], "--lens", lens, "--offset", str(offset)]
    if command == "activity-detail:app":
        argv += ["--app", app]
    elif command == "activity-detail:domain":
        argv += ["--domain", domain]
    elif command == "insights":
        argv += ["--json"]
    elif command == "summary":
        argv += ["--days", str({"day": 31, "week": 14, "month": 31, "year": 90, "life": 90}[lens])]
    start = time.perf_counter()
    result = subprocess.run(argv, capture_output=True, check=True, timeout=120)
    return time.perf_counter()-start, json.loads(result.stdout), len(result.stdout)


def stable_data(report, path=()):
    # Current-period elapsed/gap values and time-relative insights change between
    # subprocesses. Historical reports are also compared separately below.
    volatile = {"generated_at", "query_end_ts", "total_elapsed_seconds",
                "total_unobserved_seconds", "elapsed_seconds", "unobserved_seconds",
                "insights", "predictions", "widget_insight", "tooltip", "status_text"}
    if isinstance(report, dict):
        return {k: stable_data(v, path + (k,)) for k, v in report.items()
                if k not in volatile and not (path == ("multitasking", "timeline") and k == "end")}
    if isinstance(report, list):
        return [stable_data(v, path) for v in report]
    return report


def historical_data(report):
    if isinstance(report, dict):
        return {k: historical_data(v) for k, v in report.items()
                if k not in {"generated_at", "widget_insight", "predictions"}}
    if isinstance(report, list):
        return [historical_data(v) for v in report]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--database", required=True)
    parser.add_argument("--config")
    parser.add_argument("--synthetic-intervals", type=int, default=0)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--app")
    parser.add_argument("--domain")
    parser.add_argument("--commands", nargs="+", default=["widget-summary", "summary", "activity-detail:app", "activity-detail:domain"],
                        choices=["widget-summary", "summary", "activity-detail:app", "activity-detail:domain", "insights"])
    args = parser.parse_args()
    if args.runs < 1 or args.synthetic_intervals < 0:
        parser.error("runs must be positive and synthetic-intervals nonnegative")
    with tempfile.TemporaryDirectory(prefix="nagori-benchmark-") as directory:
        database = Path(directory)/"snapshot.db"
        if args.synthetic_intervals:
            synthetic(args.database, database, args.synthetic_intervals)
        else:
            # A consistent private snapshot; the tracker can keep writing its DB.
            with sqlite3.connect(Path(args.database).resolve().as_uri()+"?mode=ro", uri=True) as source:
                with sqlite3.connect(database) as dest:
                    source.backup(dest)
        database.chmod(0o600)
        with sqlite3.connect(database) as conn:
            # Freeze open telemetry at its last observed heartbeat for comparison.
            row = conn.execute("SELECT COALESCE(stopped_at,last_heartbeat_at) FROM daemon_runs ORDER BY id DESC LIMIT 1").fetchone()
            if row:
                cutoff = row[0]
                for table in ("intervals", "session_intervals", "unobserved_intervals"):
                    conn.execute(f"UPDATE {table} SET ended_at=MAX(started_at,?) WHERE ended_at IS NULL", (cutoff,))
                conn.execute("UPDATE daemon_runs SET stopped_at=last_heartbeat_at,stop_kind='clean' WHERE stopped_at IS NULL")
            app = args.app or (conn.execute("SELECT app_class FROM intervals WHERE kind='focused' GROUP BY app_class ORDER BY count(*) DESC LIMIT 1").fetchone() or ["chromium"])[0]
            domain = args.domain or (conn.execute("SELECT domain FROM browser_domain_intervals GROUP BY domain ORDER BY count(*) DESC LIMIT 1").fetchone() or ["example.test"])[0]
        output = {}
        for command in args.commands:
            output[command] = {}
            for lens in ("day", "week", "month", "year", "life"):
                def run(binary, offset=0):
                    return measure(binary, database, lens, command, args.config, app, domain, offset)
                samples = {"before": [], "after": []}
                sizes = {}
                run(args.before)
                run(args.after)
                for n in range(args.runs):
                    order = [("before", args.before), ("after", args.after)]
                    if n % 2:
                        order.reverse()
                    reports = {}
                    for name, binary in order:
                        elapsed, report, size = run(binary)
                        samples[name].append(elapsed)
                        reports[name] = report
                        sizes[name] = size
                    assert stable_data(reports["before"]) == stable_data(reports["after"]), (command, lens, "data differs")
                if lens != "life":
                    assert historical_data(run(args.before, -1)[1]) == historical_data(run(args.after, -1)[1]), (command, lens, "historical data differs")
                result = {}
                for name, values in samples.items():
                    result[name] = {"median_ms": round(statistics.median(values)*1000, 2),
                                    "p95_ms": round(sorted(values)[math.ceil(len(values)*.95)-1]*1000, 2),
                                    "bytes": sizes[name]}
                result["improvement_percent"] = round((1-statistics.median(samples["after"])/statistics.median(samples["before"]))*100, 1)
                output[command][lens] = result
                print(f"{command} {lens}: {result}", file=sys.stderr, flush=True)
        print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
