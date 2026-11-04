import {describe,expect,it} from "vitest";
import {compileCrossfadeClock} from "./crossfadeClock";
import {sampleCrossfadeFrame} from "./crossfadeSampling";
import {crossfadePreviewAtTime} from "./crossfadePreviewSeek";

describe("experimental Phase J preview seek frame clock",()=>{
  const clock=compileCrossfadeClock([{id:"a",frames:10},{id:"b",frames:10}],[{
    from_clip_id:"a",to_clip_id:"b",frames:4,
  }]);
  it("samples hard-cut frames before and after a four-frame overlap",()=>{
    const before=crossfadePreviewAtTime(clock,30,5/30);
    expect(before).toMatchObject({project_frame:5,project_time_seconds:5/30,in_overlap:false});
    expect(before.sources).toMatchObject([{clip_id:"a",source_frame:5,source_offset_seconds:5/30}]);
    const after=crossfadePreviewAtTime(clock,30,10/30);
    expect(after).toMatchObject({project_frame:10,in_overlap:false});
    expect(after.sources).toMatchObject([{clip_id:"b",source_frame:4,source_offset_seconds:4/30}]);
  });
  it("maps every overlapping project frame to the same two-source sampler",()=>{
    for(let frame=6;frame<10;frame++){
      const position=crossfadePreviewAtTime(clock,30,frame/30);
      const reference=sampleCrossfadeFrame(clock,frame);
      expect(position.in_overlap).toBe(true);
      expect(position.sources.map(({source_offset_seconds,...rest})=>rest)).toEqual(reference.sources);
      expect(position.sources[0].source_offset_seconds).toBeCloseTo(frame/30);
      expect(position.sources[1].source_offset_seconds).toBeCloseTo((frame-6)/30);
    }
  });
  it("maps fractional playhead positions to discrete frames at 60fps",()=>{
    const project=compileCrossfadeClock([{id:"a",frames:60},{id:"b",frames:60}],[]);
    expect(crossfadePreviewAtTime(project,60,0.5).project_frame).toBe(30);
    expect(crossfadePreviewAtTime(project,60,0.5+0.4/60).project_frame).toBe(30);
    expect(crossfadePreviewAtTime(project,60,1).sources[0].clip_id).toBe("b");
    expect(crossfadePreviewAtTime(project,60,119/60).project_frame).toBe(119);
  });
  it("does not emit a duplicate final frame or accept non-finite timestamps",()=>{
    expect(crossfadePreviewAtTime(clock,30,15/30).project_frame).toBe(15);
    for(const seconds of [16/30,-0.001,NaN,Infinity,-Infinity]){
      expect(()=>crossfadePreviewAtTime(clock,30,seconds)).toThrow(/timestamp|duration/);
    }
    expect(()=>crossfadePreviewAtTime(clock,24 as 30,0)).toThrow(/frame rate/);
    expect(()=>crossfadePreviewAtTime(compileCrossfadeClock([],[]),30,0)).toThrow(/timestamp/);
  });
  it("does not mutate the clock and rejects inconsistent frame ranges",()=>{
    const original=JSON.stringify(clock);
    crossfadePreviewAtTime(clock,30,6/30);
    expect(JSON.stringify(clock)).toBe(original);
    expect(()=>crossfadePreviewAtTime({...clock,total_frames:999},30,998/30)).toThrow(/source clips/);
  });
});
