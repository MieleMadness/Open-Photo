// @vitest-environment node
import {it,expect} from "vitest";
import {readFileSync,readdirSync} from "node:fs";
import {join} from "node:path";
it("explicit cleanup covers databases named by native persistence services",()=>{
  const native=join(process.cwd(),"src-tauri/src");
  const databases=new Set(readdirSync(native).filter(name=>name.endsWith(".rs")&&!name.endsWith("_tests.rs")).flatMap(name=>[...readFileSync(join(native,name),"utf8").split("#[cfg(test)]")[0].matchAll(/"([a-z][a-z0-9-]*\.sqlite)"/g)].map(match=>match[1])));
  expect(databases.has("ai-work.sqlite")).toBe(true);
  const hooks=readFileSync("src-tauri/installer/hooks.nsh","utf8");
  const acceptance=readFileSync("scripts/test-installed-app.ps1","utf8");
  for(const database of databases){
    expect(hooks,`uninstall ownership missing ${database}`).toContain(`!insertmacro OpenPhotoDeleteDatabase "${database}"`);
    expect(acceptance,`installed acceptance missing ${database}`).toContain(`'${database}'`);
  }
  for(const suffix of ["-wal","-shm","-journal"])expect(hooks).toContain(`\u0024{NAME}${suffix}`);
});
