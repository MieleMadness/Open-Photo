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
it("automatically searches meaning after typing stops, with Enter as an immediate retry",async()=>{
 const {result}=renderHook(()=>useUnifiedSearch(initial));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",{request:expect.objectContaining({workspaceId:"one",query:"burger",includeSemantic:true})}));
 await act(async()=>result.current.submit());await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="search_workspace_evidence")).toHaveLength(2));
});
it("rejects late results after changing query, workspace, or the AI switch",async()=>{
 let finish:(value:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(command)=>command==="search_workspace_evidence"?new Promise(resolve=>{finish=resolve}):undefined);
 const {result,rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:initial});await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",expect.anything()));
 rerender({...initial,workspace:"other",query:"lunch",aiEnabled:false});await act(async()=>finish({evidence:[hit("burger")],warnings:[],cancelled:false}));
 expect(result.current.results.every(r=>r.reasons.every(reason=>reason.kind!=="ocr"))).toBe(true);await waitFor(()=>expect(result.current.busy).toBe(false));expect(invoke).toHaveBeenCalledWith("cancel_workspace_search",{requestId:expect.any(String)});
});
it("keeps metadata results available when an indexed provider fails",async()=>{
 vi.mocked(invoke).mockRejectedValue(Error("Index unavailable"));const {result}=renderHook(()=>useUnifiedSearch({...initial,query:"one"}));
 await waitFor(()=>expect(result.current.warnings.join(" ")).toContain("Index unavailable"));expect(result.current.results).toHaveLength(1);expect(result.current.results[0].reasons[0].kind).toBe("filename");
});
it("does not query native AI indexes when AI is off or the search has only metadata filters",async()=>{
 const {rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:{...initial,aiEnabled:false}});await act(async()=>{await new Promise(resolve=>setTimeout(resolve,350));});expect(invoke).not.toHaveBeenCalled();rerender({...initial,query:"is:favorite",aiEnabled:true});await act(async()=>{await new Promise(resolve=>setTimeout(resolve,350));});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="search_workspace_evidence")).toBe(false);
});
it("keeps the last completed view during a new search and rejects obsolete responses",async()=>{
 let finish:(value:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command!=="search_workspace_evidence")return undefined;
  return (args as {request:{query:string}}).request.query==="lunch"?new Promise(resolve=>{finish=resolve}):{evidence:[hit("burger receipt")],warnings:[],cancelled:false};
 });
 const {result,rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:initial});await waitFor(()=>expect(result.current.busy).toBe(false));expect(result.current.results[0].reasons.some(r=>r.kind==="ocr")).toBe(true);
 rerender({...initial,query:"lunch"});expect(result.current.busy).toBe(true);expect(result.current.results[0].reasons.some(r=>r.kind==="ocr")).toBe(true);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("search_workspace_evidence",{request:expect.objectContaining({query:"lunch"})}));
 await act(async()=>finish({evidence:[],warnings:[],cancelled:false}));expect(result.current.busy).toBe(false);expect(result.current.results).toHaveLength(0);
});
it("coalesces rapid typing into one semantic request",async()=>{
 const {result,rerender}=renderHook(props=>useUnifiedSearch(props),{initialProps:initial});rerender({...initial,query:"burge"});rerender({...initial,query:"burger meal"});
 await waitFor(()=>expect(result.current.busy).toBe(false));const requests=vi.mocked(invoke).mock.calls.filter(([c])=>c==="search_workspace_evidence");expect(requests).toHaveLength(1);expect(requests[0][1]).toMatchObject({request:{query:"burger meal",includeSemantic:true}});
});
