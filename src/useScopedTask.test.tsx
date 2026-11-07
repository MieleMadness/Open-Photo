import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {useScopedTask} from "./useScopedTask";
afterEach(cleanup);
function deferred<T>(){let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}
it("rejects late successes and errors even after switching back to the original workspace",async()=>{
  const a=deferred<string>(),b=deferred<string>();const {result,rerender}=renderHook(scope=>useScopedTask<string>(scope),{initialProps:"A"});
  let old!:Promise<void>;act(()=>{old=result.current.run(()=>a.promise);});rerender("B");expect(result.current.busy).toBe(false);
  let newer!:Promise<void>;act(()=>{newer=result.current.run(()=>b.promise);});
  await act(async()=>{a.resolve("old A");await old;});expect(result.current.busy).toBe(true);expect(result.current.value).toBeNull();
  rerender("A");await act(async()=>{b.reject(Error("old B"));await newer;});expect(result.current.error).toBe("");expect(result.current.busy).toBe(false);
});
it("reset and a newer request own finalization; a late reply cannot replace them",async()=>{
  const a=deferred<number>(),b=deferred<number>();const {result}=renderHook(()=>useScopedTask<number>("A"));
  let first!:Promise<void>,second!:Promise<void>;act(()=>{first=result.current.run(()=>a.promise);result.current.reset();second=result.current.run(()=>b.promise);});
  await act(async()=>{a.reject(Error("obsolete"));await first;});expect(result.current.busy).toBe(true);expect(result.current.error).toBe("");
  await act(async()=>{b.resolve(2);await second;});expect(result.current.value).toBe(2);
});
