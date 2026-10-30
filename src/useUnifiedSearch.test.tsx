import {act,cleanup,renderHook,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {useUnifiedSearch} from "./useUnifiedSearch";
import type {SearchEvidence,SearchMedia} from "./unifiedSearch";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));vi.mock("./userData",()=>({flushSettings:vi.fn(async()=>{})}));
const photos:SearchMedia[]=[{path:"C:/A/one.jpg",name:"one.jpg",extension:"jpg",modified_unix:10,libraryRoot:"C:/A"}];
const initial={photos,root:"C:/A",workspace:"one",query:"burger",tags:{},ratings:{},aiEnabled:true,desktop:true,device:"cpu",indexing:false,refresh:0};
const hit=(text:string):SearchEvidence=>({root:"C:/A",path:photos[0].path,modified_unix:10,kind:"ocr",text,score:1,timestamp_seconds:null});
beforeEach(()=>vi.mocked(invoke).mockResolvedValue({evidence:[],warnings:[],cancelled:false}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
it("uses cached content while typing and runs meaning inference only after submission",async()=>{
 const {result}=renderHook(()=>useUnifiedSearch(initial));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",{request:expect.objectContaining({workspaceId:"one",query:"burger",includeSemantic:false})}));
 await act(async()=>result.current.submit());await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",{request:expect.objectContaining({includeSemantic:true})}));
});
it("rejects late results after changing query, workspace, or the AI switch",async()=>{
 let finish:(value:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(command)=>command==="search_workspace_evidence"?new Promise(resolve=>{finish=resolve}):undefined);
 const {result,rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:initial});await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",expect.anything()));
 rerender({...initial,workspace:"other",query:"lunch",aiEnabled:false});await act(async()=>finish({evidence:[hit("burger")],warnings:[],cancelled:false}));
 expect(result.current.results).toEqual([]);expect(result.current.busy).toBe(false);expect(invoke).toHaveBeenCalledWith("cancel_workspace_search",{requestId:expect.any(String)});
});
it("keeps metadata results available when an indexed provider fails",async()=>{
 vi.mocked(invoke).mockRejectedValue(Error("Index unavailable"));const {result}=renderHook(()=>useUnifiedSearch({...initial,query:"one"}));
 await waitFor(()=>expect(result.current.warnings.join(" ")).toContain("Index unavailable"));expect(result.current.results).toHaveLength(1);expect(result.current.results[0].reasons[0].kind).toBe("filename");
});
it("does not query native AI indexes when AI is off or the search has only metadata filters",async()=>{
 const {rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:{...initial,aiEnabled:false}});await act(async()=>{await new Promise(resolve=>setTimeout(resolve,350));});expect(invoke).not.toHaveBeenCalled();rerender({...initial,query:"is:favorite",aiEnabled:true});await act(async()=>{await new Promise(resolve=>setTimeout(resolve,350));});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="search_workspace_evidence")).toBe(false);
});
it("retains current cached matches while adding an explicit meaning search",async()=>{
 let finish:(value:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command!=="search_workspace_evidence")return undefined;
  return (args as {request:{includeSemantic:boolean}}).request.includeSemantic?new Promise(resolve=>{finish=resolve}):{evidence:[hit("burger receipt")],warnings:[],cancelled:false};
 });
 const {result}=renderHook(()=>useUnifiedSearch(initial));await waitFor(()=>expect(result.current.results).toHaveLength(1));await act(async()=>result.current.submit());await waitFor(()=>expect(result.current.busy).toBe(true));expect(result.current.results).toHaveLength(1);
 await act(async()=>finish({evidence:[hit("burger receipt")],warnings:[],cancelled:false}));expect(result.current.busy).toBe(false);
});
