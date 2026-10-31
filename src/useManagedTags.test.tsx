import {renderHook,waitFor,cleanup,render} from "@testing-library/react";
import {afterEach,beforeEach,describe,it,expect,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {useManagedTags} from "./useManagedTags";
import App from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(path:string)=>path}));
vi.mock("@tauri-apps/api/event",()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock("./PhotoMap",()=>({default:()=>null}));
beforeEach(()=>{
  localStorage.clear();
  vi.stubGlobal("matchMedia",vi.fn(()=>({matches:false,addEventListener:vi.fn(),removeEventListener:vi.fn()})));
});
afterEach(()=>{cleanup();vi.restoreAllMocks()});
describe("Tags lifecycle",()=>{
  it("renders the application without accessing uninitialized state",()=>{
    expect(()=>render(<App/>)).not.toThrow();
  });
  it("refreshes an active Library filter and clears another workspace's tags",async()=>{
    vi.mocked(invoke).mockResolvedValue([{path:"a/1",tags:["Old"]}]);
    const photos=[{path:"a/1",libraryRoot:"a"}];
    const {result,rerender}=renderHook(({workspace,refresh})=>useManagedTags(photos,"a",workspace,true,refresh),{initialProps:{workspace:"one",refresh:0}});
    await waitFor(()=>expect(result.current.managedTags["a/1"]).toEqual(["Old"]));
    vi.mocked(invoke).mockResolvedValue([{path:"a/1",tags:["Old","New"]}]);
    rerender({workspace:"one",refresh:1});
    await waitFor(()=>expect(result.current.managedTags["a/1"]).toContain("New"));
    vi.mocked(invoke).mockResolvedValue([]);
    rerender({workspace:"two",refresh:1});
    await waitFor(()=>expect(result.current.managedTags).toEqual({}));
  });
});
