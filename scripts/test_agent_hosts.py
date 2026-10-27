"""Probe installed bridges without credentials or global configuration.

Build with cargo build. Set AGENT_HOST_MODULES to node_modules containing
@deepseek-ai/dsh@0.2.1-rc.2; optional OPENCODE_BIN also validates real host loading.
"""
from pathlib import Path
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    binary = ROOT / "target/debug" / ("reporinse.exe" if os.name == "nt" else "reporinse")
    if not binary.is_file() or not os.environ.get("AGENT_HOST_MODULES"):
        raise SystemExit("cargo build; set AGENT_HOST_MODULES; NOT EXECUTED")
    with tempfile.TemporaryDirectory(prefix="native-hooks-") as folder:
        root = Path(folder)
        env = {**os.environ,"HOME":folder,"USERPROFILE":folder,"XDG_CONFIG_HOME":str(root / "config"),
               "XDG_CACHE_HOME":str(root / "cache"),"XDG_DATA_HOME":str(root / "data"),
               "APPDATA":str(root / "appdata"),"LOCALAPPDATA":str(root / "local"),
               "GIT_CONFIG_NOSYSTEM":"1","GIT_CONFIG_GLOBAL":str(root / "gitconfig"),
               "OPENCODE_DISABLE_AUTOUPDATE":"true","OPENCODE_DISABLE_DEFAULT_PLUGINS":"true"}
        def run(args):
            return subprocess.run(args,cwd=root,env=env,check=True,capture_output=True,text=True,encoding="utf-8",timeout=90)
        run(["git","init","-q"])
        (root / ".reporinse.toml").write_text('version=1\n[rules."agent.command-guard"]\nmode="block"\ndeny=["*never-run-this*"]\n[rules."agent.path-scope"]\nmode="block"\ndeny=["*private.key"]\n')
        for host in ("opencode","deepseek"):
            run([str(binary),"install","--provider",host,"--all","--yes"])
        subprocess.run(["node",str(ROOT / "scripts/probe_agent_bridges.mjs"),folder],cwd=root,env=env,check=True,timeout=90)
        if os.environ.get("OPENCODE_BIN"):
            # MCP discovery initializes the project without sending a model request.
            run([os.environ["OPENCODE_BIN"],"mcp","list"])
            print("OpenCode executable: project startup passed (no model turn)")
        for host in ("opencode","deepseek"):
            run([str(binary),"uninstall","--provider",host,"--all","--yes"])


if __name__ == "__main__":
    main()
