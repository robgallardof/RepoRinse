/** Contract callbacks + real RepoRinse child processes and DeepSeek tool pipeline. */
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { existsSync, writeFileSync, readFileSync } from "node:fs";
const root = process.argv[2];
const open = await import(pathToFileURL(resolve(root,".opencode/plugins/reporinse.js")).href);
const callbacks = await open.RepoRinse({directory:root});
const input = {tool:"bash",sessionID:"test-session"};
await assert.rejects(() => callbacks["tool.execute.before"](input,{args:{command:"never-run-this"}}), /policy|configured/);
await callbacks["tool.execute.before"](input,{args:{command:"safe-command"}});
await callbacks["tool.execute.after"]({...input,args:{command:"never-run-this"}},{});
await assert.rejects(() => callbacks["tool.execute.before"]({tool:"read"},{args:{filePath:"private.key"}}), /policy|configured/);
assert(!existsSync(resolve(root,"sentinel")), "payload command was executed");
console.log("OpenCode: installed callbacks invoke real CLI; before blocks, after permits, paths map correctly");

const modules = process.env.AGENT_HOST_MODULES;
assert(modules,"set AGENT_HOST_MODULES");
const load = name => import(pathToFileURL(resolve(modules,"@deepseek-ai",name,"lib/index.js")).href);
const { Context } = await load("cordis");
const { SystemPrompt } = await load("dsh-system-prompt");
const { ToolRuntime } = await load("dsh-tools");
const overlay = JSON.parse(readFileSync(resolve(root,".dsh/reporinse.cordis.yml"),"utf8"));
const plugin = await import(overlay[0].name);
const ctx = new Context();
try {
 await ctx.plugin(SystemPrompt).await();
 await ctx.plugin(ToolRuntime).await();
 await ctx.plugin(plugin).await();
 let executions = 0;
 ctx.tools.register({name:"bash",description:"Isolated integration fixture",parameters:{type:"object",properties:{command:{type:"string"}}},
   output:{schema:{type:"string"},render:(_args,value)=>[{type:"text",text:value}]},
   execute:async()=>{executions++;writeFileSync(resolve(root,"sentinel"),"executed allowed body");return "allowed";}});
 const call = command => ctx.tools.execute({callId:"fixture-call",name:"bash",arguments:{command},signal:AbortSignal.timeout(15000)});
 const denied = await call("never-run-this");
 assert.equal(denied.isError,true);
 assert.equal(executions,0);
 assert(!existsSync(resolve(root,"sentinel")));
 const allowed = await call("safe-command");
 assert.equal(allowed.isError,false,JSON.stringify(allowed));
 assert.equal(executions,1);
 console.log("DeepSeek Harness: native pipeline blocks before body; allowed body executes exactly once");
} finally {await ctx.fiber.dispose();}
