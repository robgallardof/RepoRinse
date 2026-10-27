// Native host callbacks forward structured data to RepoRinse; commands are never evaluated here.
import { spawn } from "node:child_process";

async function inspect(provider, event, tool, args, cwd, sessionId) {
  const names = {bash:"Bash",pwsh:"PowerShell",read:"Read",write:"Write",edit:"Edit",str_replace_editor:"Edit"};
  const payload = {hook_event_name:event,tool_name:names[tool] ?? tool,
    tool_input:{...args,file_path:args.filePath ?? args.path ?? args.file_path},cwd,session_id:sessionId};
  return new Promise((done) => {
    const child = spawn(SETTINGS.binary, ["hook","--only",SETTINGS.modules.join(","),provider,event],
      {cwd,stdio:["pipe","pipe","pipe"],windowsHide:true,shell:false});
    let output = "", finished = false;
    const finish = (value) => {if (!finished) {finished=true;clearTimeout(timer);done(value);}};
    const timer = setTimeout(() => {child.kill();finish({});}, 10000);
    child.on("error", () => finish({}));
    child.stdin.on("error", () => finish({}));
    child.stderr.resume(); // Do not copy commands or paths into the host's output.
    child.stdout.on("data", chunk => {output+=chunk; if(output.length>1048576){child.kill();finish({});}});
    child.on("close", code => {try {finish(code === 0 && output.trim() ? JSON.parse(output) : {});} catch {finish({});}});
    child.stdin.end(JSON.stringify(payload));
  });
}

/** OpenCode 1.x auto-loads the named plugin from .opencode/plugins. */
export const RepoRinse = async ({directory}) => ({
  "tool.execute.before": async (input, output) => {
    const result = await inspect("opencode","PreToolUse",input.tool,output.args ?? {},directory,input.sessionID);
    if(result.hookSpecificOutput?.permissionDecision === "deny")
      throw new Error(result.hookSpecificOutput.permissionDecisionReason || "RepoRinse policy denied this action");
  },
  "tool.execute.after": async (input) => {
    await inspect("opencode","PostToolUse",input.tool,input.args ?? {},directory,input.sessionID);
  }
});

/** DeepSeek Harness uses its native tools/pre-execute waterfall. */
export const name = "reporinse";
export const inject = ["tools"];
export async function apply(ctx) {
  ctx.on("tools/pre-execute", async (exec, next) => {
    // Session working-directory support is optional; the installed repository is the fallback.
    const cwd = SETTINGS.root;
    const result = await inspect("deepseek","PreToolUse",exec.name,exec.arguments ?? {},cwd);
    if(result.hookSpecificOutput?.permissionDecision === "deny")
      return {kind:"deny",reason:result.hookSpecificOutput.permissionDecisionReason || "RepoRinse policy denied this action"};
    return next();
  });
}
