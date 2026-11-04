import {describe,expect,it} from "vitest";
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";
import {reconcileCrossfadeCuts} from "./crossfadeReconciliation";

const clips:CrossfadeClipClock[]=[
  {id:"a",frames:60},{id:"b",frames:40},{id:"c",frames:60},
];
const ab:CrossfadeBoundary={from_clip_id:"a",to_clip_id:"b",frames:10};
const bc:CrossfadeBoundary={from_clip_id:"b",to_clip_id:"c",frames:10};

describe("experimental crossfade cut reconciliation",()=>{
  it("preserves valid adjacent transitions and canonical timeline order",()=>{
    const input=[bc,ab];
    const output=reconcileCrossfadeCuts(clips,input);
    expect(output).toEqual({boundaries:[ab,bc],removed:[]});
    expect(input).toEqual([bc,ab]);
    expect(compileCrossfadeClock(clips,output.boundaries).total_frames).toBe(140);
  });
  it("drops stale transitions on move and delete while keeping a still-valid pair",()=>{
    const reordered=[clips[1],clips[2],clips[0]];
    expect(reconcileCrossfadeCuts(reordered,[ab,bc])).toEqual({boundaries:[bc],removed:[ab]});
    expect(reconcileCrossfadeCuts([clips[0],clips[2]],[ab,bc])).toEqual({boundaries:[],removed:[ab,bc]});
    expect(reconcileCrossfadeCuts([],[])).toEqual({boundaries:[],removed:[]});
  });
  it("drops a transition instead of silently shortening it after a trim",()=>{
    const shorter=[{...clips[0],frames:8},clips[1],clips[2]];
    expect(reconcileCrossfadeCuts(shorter,[ab,bc])).toEqual({boundaries:[bc],removed:[ab]});
  });
  it("keeps earlier cuts if neighboring overlap budgets become incompatible",()=>{
    const shorter=[clips[0],{...clips[1],frames:18},clips[2]];
    expect(reconcileCrossfadeCuts(shorter,[bc,ab])).toEqual({boundaries:[ab],removed:[bc]});
  });
  it("rejects malformed or duplicate transition edges but reports every removal",()=>{
    const invalid=[{...ab,frames:0},{...ab,frames:10},{...ab,frames:10},{...bc,frames:Infinity}];
    expect(reconcileCrossfadeCuts(clips,invalid)).toEqual({
      boundaries:[ab],
      removed:[invalid[0],invalid[2],invalid[3]],
    });
  });
  it("rejects invalid clip layouts rather than accidentally retaining corrupted state",()=>{
    expect(()=>reconcileCrossfadeCuts([{id:"a",frames:1}],[])).toThrow(/at least two/);
    expect(()=>reconcileCrossfadeCuts([clips[0],clips[0]],[])).toThrow(/unique IDs/);
  });
});
