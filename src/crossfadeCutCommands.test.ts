import {describe,expect,it} from "vitest";
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";
import {maxCrossfadeFrames,setCrossfadeAtCut} from "./crossfadeCutCommands";

const clips:CrossfadeClipClock[]=[
  {id:"a",frames:90},{id:"b",frames:60},{id:"c",frames:90},
];
const ab:CrossfadeBoundary={from_clip_id:"a",to_clip_id:"b",frames:12};
const bc:CrossfadeBoundary={from_clip_id:"b",to_clip_id:"c",frames:15};

describe("Phase J crossfade cut editing (not production enabled)",()=>{
  it("creates and replaces a cut in stable project order without changing source objects",()=>{
    const original=[bc];
    const first=setCrossfadeAtCut(clips,original,"a",12,30);
    expect(first).toEqual({changed:true,boundaries:[ab,bc]});
    expect(original).toEqual([bc]);
    const updated=setCrossfadeAtCut(clips,first.boundaries,"a",18,30);
    expect(updated.boundaries).toEqual([{...ab,frames:18},bc]);
    expect(compileCrossfadeClock(clips,updated.boundaries).total_frames).toBe(207);
  });
  it("treats unchanged and missing transition removal as no-op and removes requested cuts",()=>{
    expect(setCrossfadeAtCut(clips,[ab],"a",12,30)).toEqual({changed:false,boundaries:[ab]});
    expect(setCrossfadeAtCut(clips,[ab],"b",null,30)).toEqual({changed:false,boundaries:[ab]});
    expect(setCrossfadeAtCut(clips,[ab,bc],"a",null,30)).toEqual({changed:true,boundaries:[bc]});
  });
  it("respects both remaining clip budgets and adjacent transitions",()=>{
    const short=[{id:"a",frames:20},{id:"b",frames:25},{id:"c",frames:20}];
    const later={...bc,frames:14};
    expect(maxCrossfadeFrames(short,[later],"a",30)).toBe(9);
    expect(setCrossfadeAtCut(short,[later],"a",9,30).boundaries).toHaveLength(2);
    expect(()=>setCrossfadeAtCut(short,[later],"a",10,30)).toThrow(/exhaust/);
    expect(maxCrossfadeFrames([{id:"a",frames:3},{id:"b",frames:20}],[],"a",30)).toBe(0);
  });
  it("permits reducing an existing cut and computes its cap without self-counting",()=>{
    const short=[{id:"a",frames:20},{id:"b",frames:25}];
    const original=[{...ab,frames:15}];
    expect(maxCrossfadeFrames(short,original,"a",30)).toBe(18);
    expect(setCrossfadeAtCut(short,original,"a",18,30).boundaries[0].frames).toBe(18);
    expect(setCrossfadeAtCut(short,original,"a",3,30).boundaries[0].frames).toBe(3);
  });
  it("rejects stale cut identities, malformed durations and unsupported rates",()=>{
    expect(()=>setCrossfadeAtCut(clips,[],"missing",9,30)).toThrow(/no longer exists/);
    expect(()=>setCrossfadeAtCut(clips,[],"c",9,30)).toThrow(/no longer exists/);
    for(const value of [0,1,2,46,0.5,Number.NaN,Infinity]){
      expect(()=>setCrossfadeAtCut(clips,[],"a",value,30)).toThrow(/duration/);
    }
    expect(()=>setCrossfadeAtCut(clips,[],"a",5,24 as 30)).toThrow(/frame rate/);
    expect(()=>maxCrossfadeFrames(clips,[],"a",24 as 30)).toThrow(/frame rate/);
  });
  it("rejects invalid preexisting state instead of silently discarding transitions",()=>{
    const invalid=[{...ab,to_clip_id:"c"}];
    expect(()=>setCrossfadeAtCut(clips,invalid,"a",null,30)).toThrow(/adjacent/);
    expect(()=>maxCrossfadeFrames(clips,invalid,"a",30)).toThrow(/adjacent/);
  });
  it("uses 60fps minimum and maximum whole-frame duration limits",()=>{
    const large=[{id:"a",frames:200},{id:"b",frames:200}];
    expect(maxCrossfadeFrames(large,[],"a",60)).toBe(90);
    expect(setCrossfadeAtCut(large,[],"a",5,60).boundaries[0].frames).toBe(5);
    expect(()=>setCrossfadeAtCut(large,[],"a",4,60)).toThrow(/duration/);
    expect(()=>setCrossfadeAtCut(large,[],"a",91,60)).toThrow(/duration/);
  });
});
