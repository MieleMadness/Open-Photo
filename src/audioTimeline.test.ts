import {describe,it,expect} from "vitest";
import {placeAudioAtPlayhead,trimAudioEndAtPlayhead} from "./audioTimeline";
import type {AudioTrack} from "./videoSequence";

const track:AudioTrack={id:"test",asset_id:"music",start_seconds:1,source_in_seconds:2,duration_seconds:5,gain_percent:80,fade_in_seconds:1,fade_out_seconds:1};

describe("audio timeline playhead actions",()=>{
  it("trims the audio endpoint while preserving source offset, volume and gain",()=>{
    const result=trimAudioEndAtPlayhead(track,3,20,15);
    expect(result).toEqual({...track,duration_seconds:2});
    expect(track.duration_seconds).toBe(5);
  });
  it("clamps both fades when trimming to a very short segment",()=>{
    const result=trimAudioEndAtPlayhead(track,1.5,20,15);
    expect(result).toEqual({...track,duration_seconds:0.5,fade_in_seconds:0.5,fade_out_seconds:0});
  });
  it("rejects too-short and out-of-bounds audio trims",()=>{
    expect(trimAudioEndAtPlayhead(track,1.01,20,15)).toBeNull();
    expect(trimAudioEndAtPlayhead(track,11,9,15)).toBeNull();
    expect(trimAudioEndAtPlayhead(track,16,20,15)).toBeNull();
  });
  it("moves a layer to the playhead without changing its source trim",()=>{
    expect(placeAudioAtPlayhead(track,3.25,10)).toEqual({...track,start_seconds:3.25});
    expect(placeAudioAtPlayhead(track,7,10)).toBeNull();
    expect(placeAudioAtPlayhead(track,-1,10)).toBeNull();
  });
});
