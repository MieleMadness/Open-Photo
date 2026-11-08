import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {TransitionSession} from "./transitionSession";
import {discardProjectSaves,flushProjects,hasUnsavedProjects} from "./projectSession";
import {defaultEdits} from "./videoProject";
import {type SequenceSession,type SequenceProject} from "./videoSequence";
import {saveTransitionDraft,type TransitionState} from "./workspaceTransitions";
vi.mock("./workspaceTransitions",()=>({saveTransitionDraft:vi.fn()}));
function source(){const project:SequenceProject={schema_version:3,id:"p",title:"Project",frame_rate:30,output:{width:96,height:64,fit:"fit"},assets:[{id:"photo",relative_path:"photo.png",bytes:100,sha256:"0".repeat(64),media:{kind:"photo",width:96,height:64}}],clips:["first","second"].map(id=>({id,asset_id:"photo",source_in:{ticks:"0",time_base:"1/1000000"},source_out:{ticks:"3000000",time_base:"1/1000000"},edits:defaultEdits()}))};return {root:"/library",workspaceId:"one",revision:7,history:{present:project},flush:vi.fn(async()=>{})} as unknown as SequenceSession;}
function state(revision=0,frames=6):TransitionState{return {project:{revision:7,hash:"a".repeat(64)},revision,record:revision?{schema_version:1,timing_policy:1,workspace:"one",project_id:"p",revision,project:{revision:7,hash:"a".repeat(64)},edges:[{from_clip_id:"first",to_clip_id:"second",frames}],recovered_from:null}:null,stale:false,history:[],timing:{fps:30,total_frames:180-(revision?frames:0),total_samples:288000-(revision?frames*1600:0),clips:[],titles:[],audio:[]},seek:null,portable:null};}
beforeEach(()=>vi.mocked(saveTransitionDraft).mockReset());afterEach(discardProjectSaves);
it("serializes native receipts and coalesces newer edits through the shared close guard",async()=>{
  let finish!:(value:TransitionState)=>void;
  vi.mocked(saveTransitionDraft).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;})).mockResolvedValueOnce(state(2,12));
  const s=source(),owner=new TransitionSession(s,state());
  owner.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}]});
  owner.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:12}]});
  expect(hasUnsavedProjects()).toBe(true);expect(saveTransitionDraft).toHaveBeenCalledTimes(1);
  finish(state(1,6));await flushProjects();
  expect(saveTransitionDraft).toHaveBeenNthCalledWith(2,s,state(1,6),[{from_clip_id:"first",to_clip_id:"second",frames:12}]);
  expect(owner.revision).toBe(2);expect(owner.saved.revision).toBe(2);expect(hasUnsavedProjects()).toBe(false);owner.release();
});
it("retains failed edits and prevents close until an explicit retry saves them",async()=>{
  vi.mocked(saveTransitionDraft).mockRejectedValueOnce(Error("disk unavailable")).mockResolvedValueOnce(state(1));
  const owner=new TransitionSession(source(),state());owner.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}]});
  await expect(flushProjects()).rejects.toThrow(/disk unavailable/);expect(owner.history.present.edges[0].frames).toBe(6);expect(hasUnsavedProjects()).toBe(true);
  owner.retry();await flushProjects();expect(owner.status).toBe("saved");expect(owner.revision).toBe(1);owner.release();
});
it("preserves the proposal when its source project changes and requires explicit rebase",()=>{
  const s=source(),owner=new TransitionSession(s,state());s.history.present={...s.history.present,title:"Changed"};
  expect(owner.stale).toBe(true);expect(()=>owner.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}]})).toThrow(/rebase/);expect(saveTransitionDraft).not.toHaveBeenCalled();owner.release();
});
it("rejects competing dips and layer overruns without enqueueing a partial edit",()=>{
  const s=source();s.history.present.transitions=[{from_clip_id:"first",to_clip_id:"second",kind:"dip_to_black",duration_seconds:0.2}];
  const owner=new TransitionSession(s,state());expect(()=>owner.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}]})).toThrow(/dip/);owner.release();
  const other=source();other.history.present.titles=[{id:"title",text:"End",start_seconds:5,end_seconds:6,x_percent:50,y_percent:50}];
  const titled=new TransitionSession(other,state());expect(()=>titled.commit({edges:[{from_clip_id:"first",to_clip_id:"second",frames:6}]})).toThrow(/title/);expect(saveTransitionDraft).not.toHaveBeenCalled();titled.release();
});
