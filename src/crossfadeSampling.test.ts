import {expect,it} from "vitest";
import {compileCrossfadeClock} from "./crossfadeClock";
import {sampleCrossfadeFrame} from "./crossfadeSampling";

it("preserves hard-cut source frames and full video/audio weights",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:5},{id:"b",frames:4}],[]);
  expect(sampleCrossfadeFrame(clock,0)).toEqual({project_frame:0,sources:[{clip_id:"a",source_frame:0,video_weight:1,audio_weight:1}]});
  expect(sampleCrossfadeFrame(clock,4).sources[0].source_frame).toBe(4);
  expect(sampleCrossfadeFrame(clock,5)).toEqual({project_frame:5,sources:[{clip_id:"b",source_frame:0,video_weight:1,audio_weight:1}]});
  expect(sampleCrossfadeFrame(clock,8).sources[0].source_frame).toBe(3);
});
it("samples both clips at exact project-frame positions with normalized visual alpha",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:10},{id:"b",frames:10}],[{from_clip_id:"a",to_clip_id:"b",frames:4}]);
  expect(clock.total_frames).toBe(16);
  expect(sampleCrossfadeFrame(clock,5).sources.map(s=>s.clip_id)).toEqual(["a"]);
  const first=sampleCrossfadeFrame(clock,6).sources;
  expect(first.map(s=>s.source_frame)).toEqual([6,0]);
  expect(first[0].video_weight).toBeCloseTo(.875);
  expect(first[1].video_weight).toBeCloseTo(.125);
  expect(first[0].video_weight+first[1].video_weight).toBeCloseTo(1);
  expect(first[0].audio_weight**2+first[1].audio_weight**2).toBeCloseTo(1);
  const last=sampleCrossfadeFrame(clock,9).sources;
  expect(last.map(s=>s.source_frame)).toEqual([9,3]);
  expect(last[0].video_weight).toBeCloseTo(.125);
  expect(last[1].video_weight).toBeCloseTo(.875);
  expect(sampleCrossfadeFrame(clock,10).sources).toHaveLength(1);
  expect(sampleCrossfadeFrame(clock,10).sources[0].source_frame).toBe(4);
});
it("keeps two separate crossfades without simultaneous three-source composites",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:10},{id:"b",frames:10},{id:"c",frames:10}],[
    {from_clip_id:"a",to_clip_id:"b",frames:3},
    {from_clip_id:"b",to_clip_id:"c",frames:3},
  ]);
  expect(clock.total_frames).toBe(24);
  expect(Array.from({length:clock.total_frames},(_,frame)=>sampleCrossfadeFrame(clock,frame).sources.length).filter(n=>n===2)).toHaveLength(6);
  expect(sampleCrossfadeFrame(clock,14).sources.map(s=>s.clip_id)).toEqual(["b","c"]);
});
it("rejects invalid frame coordinates and malformed source overlap layout",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:3}],[]);
  for(const frame of [-1,3,Infinity,NaN,0.5])expect(()=>sampleCrossfadeFrame(clock,frame)).toThrow(/outside/);
  expect(()=>sampleCrossfadeFrame({...clock,total_frames:4},3)).toThrow(/one or two/);
});
