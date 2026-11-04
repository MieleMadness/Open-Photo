import {expect,it} from "vitest";
import {compileMarkedCrossfadeClock,crossfadeDurationFrames,type CrossfadeMarkedClip} from "./crossfadeProjectClock";
import {markTime} from "./videoTime";

const clip=(id:string,seconds:number):CrossfadeMarkedClip=>({
  id,source_in:markTime(0,"1/1000000"),source_out:markTime(seconds,"1/1000000"),
});

it("matches native cumulative project-frame rounding instead of per-clip rounding",()=>{
  const clock=compileMarkedCrossfadeClock([clip("a",.08),clip("b",.08),clip("c",.08)],30,[]);
  // 2.4, 4.8, 7.2 total frames at successive cut boundaries.
  expect(clock.entries.map(c=>c.frames)).toEqual([2,3,2]);
  expect(clock.entries.map(c=>c.start_frame)).toEqual([0,2,5]);
  expect(clock.total_frames).toBe(7);
});
it("quantizes a crossfade to integer frames without changing source trims",()=>{
  const clips=[clip("a",2),clip("b",2),clip("c",2)];
  const frames=crossfadeDurationFrames(.5,30);
  const clock=compileMarkedCrossfadeClock(clips,30,[
    {from_clip_id:"a",to_clip_id:"b",frames},
    {from_clip_id:"b",to_clip_id:"c",frames},
  ]);
  expect(frames).toBe(15);
  expect(clock.total_frames).toBe(150);
  expect(clock.overlaps.map(e=>[e.start_frame,e.end_frame])).toEqual([[45,60],[90,105]]);
  expect(clock.entries.map(e=>e.frames)).toEqual([60,60,60]);
});
it("supports 60fps and quantized 0.08-second minimum",()=>{
  expect(crossfadeDurationFrames(.08,30)).toBe(2);
  expect(crossfadeDurationFrames(.08,60)).toBe(5);
  const clock=compileMarkedCrossfadeClock([clip("a",1),clip("b",1)],60,[
    {from_clip_id:"a",to_clip_id:"b",frames:5},
  ]);
  expect(clock.total_frames).toBe(115);
});
it("rejects invalid trimmed source marks and sub-two-frame clips",()=>{
  expect(()=>compileMarkedCrossfadeClock([clip("a",0)],30,[])).toThrow(/source marks/);
  expect(()=>compileMarkedCrossfadeClock([clip("a",.01)],30,[])).toThrow(/at least two/);
  expect(()=>compileMarkedCrossfadeClock([clip("a",7201)],30,[])).toThrow(/two hours/);
  const invalid={id:"a",source_in:markTime(2,"1/1000"),source_out:markTime(1,"1/1000")};
  expect(()=>compileMarkedCrossfadeClock([invalid],30,[])).toThrow(/source marks/);
  expect(()=>compileMarkedCrossfadeClock([clip("a",1)],24 as 30,[])).toThrow(/frame rate/);
});
it("rejects malformed duration requests and clip overlaps that exhaust source",()=>{
  for(const seconds of [-1,0,.079,1.501,NaN,Infinity]){
    expect(()=>crossfadeDurationFrames(seconds,30)).toThrow(/duration/);
  }
  expect(()=>compileMarkedCrossfadeClock([clip("a",.1),clip("b",.1)],30,[
    {from_clip_id:"a",to_clip_id:"b",frames:2},
  ])).toThrow(/available clip frames/);
});
