#!/usr/bin/env python3
"""Continuous task-driven implementation, review, tests, merge and acceptance."""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]))
from tools.delivery import engine, model, runtime, jobs
from tools.delivery.store import Store


def main():
    p=argparse.ArgumentParser(description=__doc__)
    sub=p.add_subparsers(dest="command",required=True)
    init=sub.add_parser("init",help="initialize outside-tree durable state")
    init.add_argument("--repo",default="."); init.add_argument("--state")
    init.add_argument("--config",required=True)
    for name in ("status","import","run","retry","cancel","doctor","approve-policy"):
        q=sub.add_parser(name); q.add_argument("--state",required=True)
        if name=="import": q.add_argument("manifest")
        if name=="run":
            q.add_argument("--publish",action="store_true",help="authorize reviewed and fully tested canonical publication")
            q.add_argument("--watch",action="store_true",help="continue watching for new admitted tasks")
            q.add_argument("--max-seconds",type=int,default=0)
            q.add_argument("--tasks-file",help="watch an owner-maintained task manifest and admit new tasks")
        if name in ("retry","cancel"): q.add_argument("id")
        if name in ("retry","approve-policy"): q.add_argument("--reason",required=True)
    q=sub.add_parser("_job",help=argparse.SUPPRESS)
    q.add_argument("--state",required=True);q.add_argument("--job",required=True);q.add_argument("--token",required=True)
    args=p.parse_args()
    try:
        if args.command=="_job": return engine.worker(args.state,args.job,args.token)
        if args.command=="init":
            store=engine.setup(args.repo,args.state,json.loads(pathlib.Path(args.config).read_text()))
            print(json.dumps({"state":str(store.root),"repo":store.meta("repo"),"workers":store.meta("config")["workers"]}))
            return 0
        store=Store(args.state)
        if not store.meta("repo"): raise ValueError("state is not initialized")
        if args.command=="status": print(json.dumps(engine.status(store),indent=2));return 0
        if args.command=="import":
            value=json.loads(pathlib.Path(args.manifest).read_text())
            tasks=value if isinstance(value,list) else value["tasks"]
            print(json.dumps({"admitted":store.admit(tasks,store.meta("config"))}));return 0
        if args.command=="retry": store.retry(args.id,args.reason);return 0
        if args.command=="cancel": store.cancel(args.id);return 0
        if args.command=="run":
            if args.max_seconds<0: raise ValueError("max-seconds cannot be negative")
            return engine.supervise(store,publish=args.publish,watch=args.watch,max_seconds=args.max_seconds,tasks_file=args.tasks_file)
        if args.command=="approve-policy":
            with engine.repository_control(store.meta("repo")):
                if store.jobs({"running","finished"}) or store.meta("publication_barrier") or store.meta("stop_new_publications"):
                    raise ValueError("cannot reapprove policy while work/publication needs reconciliation")
                source=jobs.Git(store.meta("repo"));cfg=store.meta("config")
                if source.run("status","--porcelain=v1") or source.run("symbolic-ref","--quiet","--short","HEAD")!=cfg["branch"]:
                    raise ValueError("policy approval requires clean canonical on the configured branch")
                if not args.reason.strip():
                    raise ValueError("policy approval requires a substantive reason")
                sha=source.head(cfg["branch"]);store.meta("policy_base_sha",sha)
                with store.connection(write=True) as cx:
                    store.event(cx,"policy.owner_approved",{"sha":sha,"reason":args.reason})
            print(json.dumps({"policy_base_sha":sha}));return 0
        if args.command=="doctor":
            cfg=store.meta("config"); out=engine.status(store)
            out["source_gate_issues"]=jobs.source_gate(store)
            if cfg["runtime"]["kind"]=="opencode":
                inv=runtime.inventory(cfg["runtime"]["executable"],store.meta("repo"),
                    required=[r["agent"] for r in cfg["routes"]], forward_env=cfg["runtime"].get("forward_env", []))
                out["missing_routes"]=[r["agent"] for r in cfg["routes"] if r["agent"] not in inv]
            else: out["runtime"]="explicit command adapter (not an OpenCode provider verification)"
            print(json.dumps(out,indent=2))
            return 2 if out.get("missing_routes") or out["source_gate_issues"] else 0
    except (ValueError, runtime.Failure, OSError) as exc:
        print(json.dumps({"error":runtime.redact(str(exc))}),file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        return 130
    return 2


if __name__=="__main__":
    raise SystemExit(main())
