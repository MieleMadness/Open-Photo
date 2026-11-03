import {expect,it} from "vitest";
import {compileCrossfadeClock,crossfadeFrameSeconds} from "./crossfadeClock";

it("builds a deterministic integer frame clock with a single true overlap",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:90},{id:"b",frames:120}],[{from_clip_id:"a",to_clip_id:"b",frames:15}]);
  expect(clock.entries).toEqual([
    {id:"a",start_frame:0,end_frame:90,frames:90},
    {id:"b",start_frame:75,end_frame:195,frames:120},
  ]);
  expect(clock.overlaps).toEqual([{from_clip_id:"a",to_clip_id:"b",start_frame:75,end_frame:90,frames:15}]);
  expect(clock.total_frames).toBe(195);
  expect(crossfadeFrameSeconds(clock.total_frames,30)).toBe(6.5);
});

it("supports sequential overlapping cuts without triple compositing",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:60},{id:"b",frames:50},{id:"c",frames:70}],[
    {from_clip_id:"a",to_clip_id:"b",frames:12},
    {from_clip_id:"b",to_clip_id:"c",frames:18},
  ]);
  expect(clock.entries.map(e=>[e.start_frame,e.end_frame])).toEqual([[0,60],[48,98],[80,150]]);
  expect(clock.overlaps.map(e=>[e.start_frame,e.end_frame])).toEqual([[48,60],[80,98]]);
  expect(clock.total_frames).toBe(150);
});
it("keeps hard cuts and empty projects backwards compatible",()=>{
  expect(compileCrossfadeClock([],[])).toEqual({entries:[],overlaps:[],total_frames:0});
  const clock=compileCrossfadeClock([{id:"a",frames:30},{id:"b",frames:30}],[]);
  expect(clock.entries[1].start_frame).toBe(30);
  expect(clock.overlaps).toEqual([]);
  expect(clock.total_frames).toBe(60);
});
it("rejects duplicate, nonadjacent, fractional and exhausted transitions",()=>{
  const clips=[{id:"a",frames:30},{id:"b",frames:20},{id:"c",frames:30}];
  const edge={from_clip_id:"a",to_clip_id:"b",frames:10};
  expect(()=>compileCrossfadeClock([{id:"a",frames:30},{id:"a",frames:40}],[])).toThrow(/unique IDs/);
  expect(()=>compileCrossfadeClock([{id:"a",frames:0}],[])).toThrow(/at least two/);
  expect(()=>compileCrossfadeClock(clips,[{from_clip_id:"a",to_clip_id:"c",frames:10}])).toThrow(/adjacent/);
  expect(()=>compileCrossfadeClock(clips,[{...edge,frames:1.2}])).toThrow(/whole project frames/);
  expect(()=>compileCrossfadeClock(clips,[edge,edge])).toThrow(/exactly once/);
  expect(()=>compileCrossfadeClock(clips,[edge,{from_clip_id:"b",to_clip_id:"c",frames:9}])).toThrow(/available clip frames/);
  expect(()=>compileCrossfadeClock(clips,[{...edge,frames:29}])).toThrow(/available clip frames/);
  expect(()=>compileCrossfadeClock(clips,[{...edge,frames:Infinity}])).toThrow(/whole project frames/);
});
it("uses one project frame rate for display without introducing fractional stored frames",()=>{
  expect(crossfadeFrameSeconds(59,60)).toBeCloseTo(0.9833333333);
  expect(crossfadeFrameSeconds(30,30)).toBe(1);
  expect(()=>crossfadeFrameSeconds(-1,30)).toThrow();
  expect(()=>crossfadeFrameSeconds(0.5,60)).toThrow();
});
