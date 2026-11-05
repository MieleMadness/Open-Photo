import {describe,expect,it} from "vitest";
import {encodeCrossfadeDraftSnapshot,type CrossfadeDraftSnapshotContext} from "./crossfadeDraftSnapshot";
import {finalizeCrossfadeDraftWorkflow,openCrossfadeDraftWorkflow,updateCrossfadeDraftWorkflow} from "./crossfadeDraftWorkflow";

const context=():CrossfadeDraftSnapshotContext=>{
  const source_clips=["a","b","c"].map((id,i)=>({
    clip_id:id,asset_id:"asset-"+i,sha256:(i+1).toString(16).padStart(2,"0").repeat(32),
    bytes:10000+i,
    source_in:{ticks:"0",time_base:"1/1000000"},
    source_out:{ticks:"2000000",time_base:"1/1000000"},
  }));
  return {
    project:{project_id:"project-a",fps:30,clips:source_clips.map(clip=>({id:clip.clip_id,frames:60}))},
    project_revision:7,source_clips,
  };
};
const fade={from_clip_id:"a",to_clip_id:"b",frames:12};
describe("experimental crossfade draft snapshot editing workflow",()=>{
  it("starts with no persisted draft, edits, undoes and restores the original absent state",()=>{
    const c=context();
    const initial=openCrossfadeDraftWorkflow(c,null);
    expect(finalizeCrossfadeDraftWorkflow(initial,c)).toEqual({changed:false,json:null});
    const added=updateCrossfadeDraftWorkflow(initial,{type:"set",fromClipId:"a",frames:12});
    expect(added.history.past).toHaveLength(1);
    const result=finalizeCrossfadeDraftWorkflow(added,c);
    expect(result.changed).toBe(true);
    expect(typeof result.json).toBe("string");
    const loaded=openCrossfadeDraftWorkflow(c,result.json);
    expect(loaded.history.present.boundaries).toEqual([fade]);
    const undone=updateCrossfadeDraftWorkflow(added,{type:"undo"});
    expect(finalizeCrossfadeDraftWorkflow(undone,c)).toEqual({changed:false,json:null});
    const redone=updateCrossfadeDraftWorkflow(undone,{type:"redo"});
    expect(finalizeCrossfadeDraftWorkflow(redone,c).changed).toBe(true);
  });
  it("retains exact original saved JSON when the edited state returns to its source",()=>{
    const c=context(),saved=encodeCrossfadeDraftSnapshot(c,[fade]);
    const initial=openCrossfadeDraftWorkflow(c,saved);
    const changed=updateCrossfadeDraftWorkflow(initial,{type:"set",fromClipId:"a",frames:15});
    expect(finalizeCrossfadeDraftWorkflow(changed,c).changed).toBe(true);
    const undone=updateCrossfadeDraftWorkflow(changed,{type:"undo"});
    expect(finalizeCrossfadeDraftWorkflow(undone,c)).toEqual({changed:false,json:saved});
  });
  it("rejects changed project revision, media identity, exact trim, frame clock and project id",()=>{
    const c=context();
    const session=updateCrossfadeDraftWorkflow(openCrossfadeDraftWorkflow(c,null),{type:"set",fromClipId:"a",frames:9});
    const variants=[
      {...context(),project_revision:8},
      {...context(),project:{...context().project,project_id:"other"}},
      {...context(),project:{...context().project,clips:[{id:"a",frames:61},...context().project.clips.slice(1)]}},
    ];
    const newSource=context();newSource.source_clips[1].sha256="ff".repeat(32);
    const newTrim=context();newTrim.source_clips[1].source_out.ticks="1999999";
    for(const candidate of [...variants,newSource,newTrim])
      expect(()=>finalizeCrossfadeDraftWorkflow(session,candidate)).toThrow(/changed/);
  });
  it("rejects invalid existing draft rather than resetting or overwriting it",()=>{
    const c=context();
    expect(()=>openCrossfadeDraftWorkflow(c,"{")).toThrow(/JSON/);
    const saved=encodeCrossfadeDraftSnapshot(c,[fade]);
    expect(()=>openCrossfadeDraftWorkflow({...c,project_revision:8},saved)).toThrow(/revision/);
    expect(()=>openCrossfadeDraftWorkflow({...c,source_clips:[...c.source_clips].reverse()},saved)).toThrow(/disagree/);
  });
  it("does not add history for a no-op or change original serialized state on errors",()=>{
    const c=context();
    const initial=openCrossfadeDraftWorkflow(c,null);
    expect(updateCrossfadeDraftWorkflow(initial,{type:"set",fromClipId:"a",frames:null})).toBe(initial);
    expect(()=>updateCrossfadeDraftWorkflow(initial,{type:"set",fromClipId:"a",frames:100})).toThrow(/duration/);
    expect(finalizeCrossfadeDraftWorkflow(initial,c)).toEqual({changed:false,json:null});
  });
  it("does not mutate caller-owned source data after opening a draft",()=>{
    const c=context(),opened=openCrossfadeDraftWorkflow(c,null);
    c.source_clips[0].bytes=2;
    c.project.clips[0].frames=2;
    expect(opened.history.present.clips[0].frames).toBe(60);
    expect(()=>finalizeCrossfadeDraftWorkflow(opened,c)).toThrow(/changed/);
  });
});
