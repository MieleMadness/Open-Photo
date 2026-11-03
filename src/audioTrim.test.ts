import {describe,expect,it} from "vitest";
import {trimAudioEdge} from "./audioTrim";
import type {AudioTrack} from "./videoSequence";

const track:AudioTrack={id:"sound",asset_id:"song",start_seconds:5,source_in_seconds:2,duration_seconds:6,gain_percent:80,fade_in_seconds:1,fade_out_seconds:2};

describe("non-destructive audio edge trims",()=>{
  it("advances source and project starts together, holding the endpoint and gain",()=>{
    const next=trimAudioEdge(track,"start",1.25,20,15);
    expect(next).toEqual({...track,start_seconds:6.25,source_in_seconds:3.25,duration_seconds:4.75});
    expect(next!.start_seconds+next!.duration_seconds).toBe(11);
    expect(track.start_seconds).toBe(5);
  });
  it("extends the start only where both source and project have preceding material",()=>{
    expect(trimAudioEdge(track,"start",-10,20,15)).toEqual({...track,start_seconds:3,source_in_seconds:0,duration_seconds:8});
  });
  it("trims or extends the end within source and project limits",()=>{
    expect(trimAudioEdge(track,"end",-2.5,20,15)).toEqual({...track,duration_seconds:3.5});
    expect(trimAudioEdge(track,"end",100,20,12)).toEqual({...track,duration_seconds:7});
  });
  it("clamps fades when shortening and keeps zero-length edits from committing",()=>{
    const next=trimAudioEdge(track,"end",-5.8,20,15);
    expect(next).toEqual({...track,duration_seconds:0.2,fade_in_seconds:0.2,fade_out_seconds:0});
    expect(trimAudioEdge(track,"end",0,20,15)).toBeNull();
    expect(trimAudioEdge(track,"end",Number.NaN,20,15)).toBeNull();
    expect(trimAudioEdge(track,"start",0,20,15)).toBeNull();
  });
  it("never allows a start before zero or trimming below the minimum",()=>{
    const atOrigin={...track,start_seconds:0,source_in_seconds:0};
    expect(trimAudioEdge(atOrigin,"start",-3,20,15)).toBeNull();
    const next=trimAudioEdge(track,"start",1000,20,15);
    expect(next!.duration_seconds).toBeCloseTo(0.04);
    expect(next!.source_in_seconds+next!.duration_seconds).toBeCloseTo(8);
  });
});
