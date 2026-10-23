#!/usr/bin/env python3
"""Offline workflow contracts only; no LLM calls, secret loading or publishing."""
import argparse, hashlib, json, os, re, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]; HEX=re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$"); PLACEHOLDER=("SET_REAL","YOUR_","CHANGEME","TODO")
def read(p): return json.loads(Path(p).read_text(encoding="utf-8"))
def config(): return read(ROOT/"agents.config.json")
def safe_path(v):
    p=(ROOT/v).resolve()
    if not p.is_relative_to(ROOT): raise ValueError("Path escapes bundle root")
    return p
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def identity(c,slot):
    i=c["models"][slot]; provider=os.environ.get(i["provider_env"],"").strip(); model=os.environ.get(i["identity_env"],"").strip()
    if not provider or not model or any(x in provider.upper() or x in model.upper() for x in PLACEHOLDER): raise ValueError("Unset model route: "+slot)
    return provider,model
def validate(c):
    if c.get("contract")!="sayl-file-workflow-v1": raise ValueError("Unknown workflow contract")
    for name in c["pipeline"]:
        r=c["roles"][name]
        if r["model"] not in c["models"]: raise ValueError("Unknown model slot: "+name)
        if not safe_path(r["prompt"]).is_file(): raise ValueError("Missing role prompt: "+name)
    for f in ("plan","debate","status"):
        if not safe_path(c["paths"][f]).is_file(): raise ValueError("Missing path: "+f)
    if c["roles"]["builder"]["model"]==c["roles"]["debater"]["model"]: raise ValueError("Builder/debater cannot share the same route")
    if c["permissions"]["public_publish"] or c["permissions"]["purchase"]: raise ValueError("Unsafe default deployment/purchase permissions")
    return {"ok":True,"validated":"bundle contract","llm_calls":0}
def check_report(c,p,require_pass=False):
    d=read(p); keys=("task_id","role","status","source_commit","source_tree","snapshot_clean","acceptance_revision","model","evidence","checks","findings","strix")
    for k in keys:
        if k not in d: raise ValueError("Missing report key: "+k)
    if d["role"] not in c["roles"] or d["status"] not in ("passed","failed","blocked"): raise ValueError("Invalid role/status")
    for k in ("evidence","checks","findings"):
        if not isinstance(d[k],list): raise ValueError("Expected list: "+k)
    for e in d["evidence"]:
        path=safe_path(e["path"])
        if not path.is_file() or sha(path)!=e["sha256"]: raise ValueError("Missing/changed evidence: "+e["path"])
    if require_pass and d["status"]!="passed": raise ValueError("Role is not passed: "+d["role"])
    if d["status"]=="passed":
        for k in ("source_commit","source_tree"):
            if not HEX.fullmatch(d[k] or ""): raise ValueError("Invalid snapshot hash: "+k)
        if not d["snapshot_clean"] or not re.fullmatch(r"[0-9a-f]{64}",d["acceptance_revision"] or ""): raise ValueError("Clean snapshot and task SHA-256 required")
        m=d["model"]
        if not m.get("provider") or not m.get("id") or not m.get("identity_verified"): raise ValueError("Verified model identity evidence required")
        if m.get("identity_source")!="runtime_metadata": raise ValueError("Passing reports need runtime metadata, not self-declared model identity")
        if not d["evidence"]: raise ValueError("Passing report requires evidence")
        for f in d["findings"]:
            if f.get("blocking") and f.get("state") not in ("closed","resolved","not_applicable_with_evidence"): raise ValueError("Open blocking finding")
        for ch in d["checks"]:
            if ch.get("required",True) and ch.get("exit_code")!=0: raise ValueError("Required command check failed or unexecuted")
        s=d["strix"]
        if d["role"]=="security_reviewer":
            if s.get("required") and not s.get("executed"): raise ValueError("Required Strix scan not executed")
            if not s.get("required") and not s.get("reason"): raise ValueError("Strix N/A requires attack-surface justification")
    return d
def git(*a):
    r=subprocess.run(["git",*a],cwd=ROOT,capture_output=True,text=True)
    if r.returncode: raise ValueError("Git snapshot unavailable: "+r.stderr.strip())
    return r.stdout.strip()
def gate(c,task_id,release):
    if not re.fullmatch(r"[A-Z0-9-]+",task_id): raise ValueError("Invalid task id")
    roles=c["required_release_reports"] if release else c["required_task_reports"]; reports=[]
    for role in roles:
        p=safe_path(c["roles"][role]["report"].format(task_id=task_id)); d=check_report(c,p,require_pass=True)
        if d["task_id"]!=task_id or d["role"]!=role: raise ValueError("Task/role mismatch")
        reports.append(d)
    if len({(d["source_commit"],d["source_tree"],d["acceptance_revision"]) for d in reports})!=1: raise ValueError("Approvals refer to different source/spec snapshots")
    spec=safe_path(c["paths"]["tasks"]+"/"+task_id+".md")
    if not spec.is_file() or sha(spec)!=reports[0]["acceptance_revision"]: raise ValueError("Task acceptance changed or missing")
    primary,challenger=identity(c,"primary"),identity(c,"challenger")
    if primary[1].casefold()==challenger[1].casefold(): raise ValueError("Builder and debater model IDs must differ")
    for d in reports:
        expected=primary if c["roles"][d["role"]]["model"]=="primary" else challenger
        if (d["model"]["provider"],d["model"]["id"])!=expected: raise ValueError("Role model route mismatch: "+d["role"])
    if git("status","--porcelain"): raise ValueError("Working tree is not clean; evidence cannot approve uncommitted changes")
    if git("rev-parse","HEAD")!=reports[0]["source_commit"] or git("rev-parse","HEAD^{tree}")!=reports[0]["source_tree"]: raise ValueError("Current repository differs from reviewed snapshot")
    return {"ok":True,"task_id":task_id,"source_snapshot_consistent":True,"publish_authorized":False,"note":"Structural gate only. Human approval and external trust remain required."}
def main():
    ap=argparse.ArgumentParser(); ap.add_argument("command",choices=["validate","preflight","packet","check-report","gate"]); ap.add_argument("arg",nargs="?"); ap.add_argument("role",nargs="?"); ap.add_argument("--release",action="store_true"); a=ap.parse_args(); c=config(); validate(c)
    if a.command=="validate": result=validate(c)
    elif a.command=="preflight":
        primary,challenger=identity(c,"primary"),identity(c,"challenger")
        if primary[1].casefold()==challenger[1].casefold(): raise ValueError("Use genuinely different model identities")
        result={"ok":True,"routes_configured":True,"provider_connection_tested":False,"budget_enforced":False,"automatic_llm_calls":False}
    elif a.command=="packet":
        if a.role not in c["roles"] or not re.fullmatch(r"[A-Z0-9-]+",a.arg or ""): raise ValueError("Usage: packet TASK-ID ROLE")
        task=safe_path(c["paths"]["tasks"]+"/"+a.arg+".md")
        if not task.is_file(): raise ValueError("Missing task specification")
        result={"task_id":a.arg,"role":a.role,"task_path":str(task.relative_to(ROOT)),"acceptance_revision":sha(task),"task":task.read_text(encoding="utf-8"),"role_prompt":safe_path(c["roles"][a.role]["prompt"]).read_text(encoding="utf-8"),"read_also":["AGENTS.md","docs/REPORT-CONTRACT.md","docs/DEBATER-SYSTEM.md"],"report_path":c["roles"][a.role]["report"].format(task_id=a.arg)}
    elif a.command=="check-report":
        d=check_report(c,safe_path(a.arg or "")); result={"ok":True,"role":d["role"],"status":d["status"],"content_truth_verified":False}
    else: result=gate(c,a.arg or "",a.release)
    print(json.dumps(result,ensure_ascii=False,indent=2))
if __name__=="__main__":
    try: main()
    except (ValueError,KeyError,TypeError,OSError,json.JSONDecodeError) as e:
        print(json.dumps({"ok":False,"error":str(e)},ensure_ascii=False),file=sys.stderr); sys.exit(1)
