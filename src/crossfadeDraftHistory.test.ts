import {describe,expect,it} from "vitest";
import {editCrossfadeDraft,startCrossfadeDraft,type CrossfadeDraft} from "./crossfadeDraftHistory";

const makeDraft=():CrossfadeDraft=>({
  fps:30,
  clips:[{id:"a",frames:60},{id:"b",frames:50},{id:"c",frames:60}],
  boundaries:[],
});

describe("Phase J in-memory crossfade history",()=>{
  it("adds, replaces and removes one cut using the common editor undo queue",()=>{
    const initial=startCrossfadeDraft(makeDraft());
    const added=editCrossfadeDraft(initial,{type:"set",fromClipId:"a",frames:9});
    expect(added.past).toHaveLength(1);
    expect(added.present.boundaries).toEqual([{from_clip_id:"a",to_clip_id:"b",frames:9}]);
    expect(initial.present.boundaries).toEqual([]);
    const changed=editCrossfadeDraft(added,{type:"set",fromClipId:"a",frames:15});
    const removed=editCrossfadeDraft(changed,{type:"set",fromClipId:"a",frames:null});
    expect(removed.present.boundaries).toEqual([]);
    const undone=editCrossfadeDraft(removed,{type:"undo"});
    expect(undone.present.boundaries[0].frames).toBe(15);
    const undoneAgain=editCrossfadeDraft(undone,{type:"undo"});
    expect(undoneAgain.present.boundaries[0].frames).toBe(9);
    const redone=editCrossfadeDraft(undoneAgain,{type:"redo"});
    expect(redone.present.boundaries[0].frames).toBe(15);
  });
  it("keeps no-op edits and empty undo as identical history objects",()=>{
    const initial=startCrossfadeDraft(makeDraft());
    expect(editCrossfadeDraft(initial,{type:"set",fromClipId:"a",frames:null})).toBe(initial);
    expect(editCrossfadeDraft(initial,{type:"undo"})).toBe(initial);
    const added=editCrossfadeDraft(initial,{type:"set",fromClipId:"a",frames:9});
    expect(editCrossfadeDraft(added,{type:"set",fromClipId:"a",frames:9})).toBe(added);
  });
  it("does not drop redo history on a no-op, but does on a new change",()=>{
    const first=editCrossfadeDraft(startCrossfadeDraft(makeDraft()),{type:"set",fromClipId:"a",frames:9});
    const undone=editCrossfadeDraft(first,{type:"undo"});
    expect(undone.future).toHaveLength(1);
    const unchanged=editCrossfadeDraft(undone,{type:"set",fromClipId:"a",frames:null});
    expect(unchanged).toBe(undone);
    const branch=editCrossfadeDraft(unchanged,{type:"set",fromClipId:"b",frames:6});
    expect(branch.future).toHaveLength(0);
    expect(branch.present.boundaries[0].from_clip_id).toBe("b");
  });
  it("leaves current history intact after invalid or over-budget edits",()=>{
    const initial=startCrossfadeDraft(makeDraft());
    for(const frames of [0,0.5,Infinity,46]){
      expect(()=>editCrossfadeDraft(initial,{type:"set",fromClipId:"a",frames})).toThrow();
    }
    expect(()=>editCrossfadeDraft(initial,{type:"set",fromClipId:"missing",frames:9})).toThrow(/no longer exists/);
    expect(initial.past).toHaveLength(0);
    expect(initial.present.boundaries).toEqual([]);
  });
  it("clones caller-owned data on entry so later mutations do not affect history",()=>{
    const draft=makeDraft();
    const initial=startCrossfadeDraft(draft);
    draft.clips[0].frames=2;
    draft.boundaries.push({from_clip_id:"a",to_clip_id:"b",frames:20});
    expect(initial.present.clips[0].frames).toBe(60);
    expect(initial.present.boundaries).toEqual([]);
  });
  it("rejects corrupt initial snapshots rather than storing unsafe transitions",()=>{
    const bad=makeDraft();
    bad.boundaries=[{from_clip_id:"a",to_clip_id:"c",frames:5}];
    expect(()=>startCrossfadeDraft(bad)).toThrow(/adjacent/);
    expect(()=>startCrossfadeDraft({...makeDraft(),fps:24 as 30})).toThrow(/frame rate/);
  });
});
