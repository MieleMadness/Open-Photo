import {beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {type SequenceSession} from "./videoSequence";
import {loadTransitionDraft,saveTransitionDraft,recoverTransitionDraft,seekTransitionDraft,transitionHistory,exportTransitionDraft,importTransitionDraft,type TransitionState} from "./workspaceTransitions";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
vi.mock("./userData",()=>({flushSettings:vi.fn(async()=>{})}));
function session(){return {workspaceId:"one",revision:7,history:{present:{id:"p"}},flush:vi.fn(async()=>{})} as unknown as SequenceSession;}
function state():TransitionState{return {project:{revision:7,hash:"a".repeat(64)},revision:0,record:null,stale:false,history:[],timing:{fps:30,total_frames:180,total_samples:288000,clips:[],titles:[],audio:[]},seek:null,portable:null};}
beforeEach(()=>{vi.clearAllMocks();vi.mocked(invoke).mockResolvedValue(state());});
it("flushes workspace and shared project saves before reading native-owned transitions",async()=>{
  const s=session();await loadTransitionDraft(s);
  expect(flushSettings).toHaveBeenCalledWith(["openphoto.workspaces"]);
  expect(s.flush).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenCalledWith("edit_workspace_video_transitions",{workspaceId:"one",action:{type:"load",id:"p"}});
  expect(vi.mocked(flushSettings).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(s.flush).mock.invocationCallOrder[0]);
  expect(vi.mocked(s.flush).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(invoke).mock.invocationCallOrder[0]);
});
it("sends exact receipts, cloned edit decisions and explicit rebase intent without roots",async()=>{
  const s=session(),loaded=state(),edges=[{from_clip_id:"first",to_clip_id:"second",frames:6}];
  const pending=saveTransitionDraft(s,loaded,edges,true);edges[0].frames=12;
  await pending;
  expect(invoke).toHaveBeenCalledWith("edit_workspace_video_transitions",{workspaceId:"one",action:{type:"save",id:"p",expected_project:{revision:7,hash:"a".repeat(64)},expected_revision:0,edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}],rebase:true}});
});
it("rejects obsolete responses when the shared project changes during native work",async()=>{
  const s=session();let finish!:(result:TransitionState)=>void;
  vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const pending=loadTransitionDraft(s);
  await vi.waitFor(()=>expect(invoke).toHaveBeenCalledOnce());
  s.history.present={...s.history.present};finish(state());
  await expect(pending).rejects.toThrow(/changed during/);
});
it("preserves unsupported records by rejecting them rather than returning an empty draft",async()=>{
  const s=session(),future={...state(),revision:1,record:{schema_version:99}};
  vi.mocked(invoke).mockResolvedValue(future);
  await expect(loadTransitionDraft(s)).rejects.toThrow(/Unsupported/);
});
it("rejects a foreign workspace record and newer project receipt",async()=>{
  const s=session(),loaded=state();
  vi.mocked(invoke).mockResolvedValue({...loaded,project:{...loaded.project,revision:8}});
  await expect(loadTransitionDraft(s)).rejects.toThrow(/changed elsewhere/);
  vi.mocked(invoke).mockResolvedValue({...loaded,revision:1,record:{schema_version:1,timing_policy:1,workspace:"other",project_id:"p",revision:1,project:loaded.project,edges:[],recovered_from:null}});
  await expect(loadTransitionDraft(s)).rejects.toThrow(/Unsupported/);
});
it("uses native paged recovery and frame seeking with the same receipts",async()=>{
  const s=session(),loaded=state();
  await transitionHistory(s,12,10);await recoverTransitionDraft(s,loaded,3,true);await seekTransitionDraft(s,loaded,{type:"frame",value:84});
  expect(invoke).toHaveBeenNthCalledWith(1,"edit_workspace_video_transitions",{workspaceId:"one",action:{type:"history",id:"p",before_revision:12,limit:10}});
  expect(invoke).toHaveBeenNthCalledWith(2,"edit_workspace_video_transitions",expect.objectContaining({action:expect.objectContaining({type:"recover",history_revision:3,rebase:true,expected_revision:0})}));
  expect(invoke).toHaveBeenNthCalledWith(3,"edit_workspace_video_transitions",expect.objectContaining({action:expect.objectContaining({type:"seek",point:{type:"frame",value:84},expected_project:loaded.project})}));
});
it("does not issue a transition request when a shared project save fails",async()=>{
  const s=session();vi.mocked(s.flush).mockRejectedValue(Error("save failed"));
  await expect(loadTransitionDraft(s)).rejects.toThrow("save failed");expect(invoke).not.toHaveBeenCalled();
});
it("loads a cleared proposal even when the empty source timeline has no transition clock",async()=>{
  vi.mocked(invoke).mockResolvedValue({...state(),timing:null});
  expect((await loadTransitionDraft(session())).timing).toBeNull();
});
it("imports and exports portable decision documents through native validation without file access grants",async()=>{
  const s=session(),loaded=state(),document='{"schema_version":1,"timing_policy":1,"project_json":"opaque","edges":[]}';
  vi.mocked(invoke).mockResolvedValue({...loaded,portable:document});
  expect((await exportTransitionDraft(s,loaded)).portable).toBe(document);
  await importTransitionDraft(s,loaded,document);
  expect(invoke).toHaveBeenNthCalledWith(1,"edit_workspace_video_transitions",expect.objectContaining({action:expect.objectContaining({type:"export",expected_project:loaded.project,expected_revision:0})}));
  expect(invoke).toHaveBeenNthCalledWith(2,"edit_workspace_video_transitions",expect.objectContaining({action:expect.objectContaining({type:"import",document,rebase:false})}));
});
