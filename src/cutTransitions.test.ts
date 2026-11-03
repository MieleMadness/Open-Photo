import {describe,expect,it} from "vitest";
import {maxCutTransitionDuration,retainAdjacentTransitions,validateCutTransitions,type CutTransition} from "./cutTransitions";
const clips=[{id:"a",duration:2},{id:"b",duration:3},{id:"c",duration:2}];
const dip:CutTransition={from_clip_id:"a",to_clip_id:"b",kind:"dip_to_black",duration_seconds:0.4};
describe("video cut transition timing",()=>{
  it("accepts an adjacent dip with unchanged clip/project durations",()=>{
    expect(validateCutTransitions(clips,[dip],30)).toBe("");
    expect(clips.reduce((sum,c)=>sum+c.duration,0)).toBe(7);
  });
  it("rejects missing, repeated and non-adjacent cut identities",()=>{
    expect(validateCutTransitions(clips,[{...dip,to_clip_id:"c"}],30)).toMatch(/adjacent/);
    expect(validateCutTransitions(clips,[dip,dip],30)).toMatch(/adjacent/);
    expect(validateCutTransitions(clips,[{...dip,from_clip_id:"x"}],30)).toMatch(/adjacent/);
  });
  it("checks fades jointly for the middle clip and rejects invalid durations",()=>{
    const short=[clips[0],{id:"b",duration:0.3},clips[2]];
    const next:CutTransition={from_clip_id:"b",to_clip_id:"c",kind:"dip_to_black",duration_seconds:0.4};
    expect(validateCutTransitions(short,[dip,next],30)).toMatch(/too short/);
    expect(validateCutTransitions(clips,[{...dip,duration_seconds:Infinity}],30)).toMatch(/duration/);
    expect(validateCutTransitions(clips,[{...dip,duration_seconds:0.01}],30)).toMatch(/duration/);
  });
  it("drops cuts whose pair is invalidated by a move or deletion",()=>{
    expect(retainAdjacentTransitions([clips[1],clips[0],clips[2]],[dip])).toEqual([]);
    expect(retainAdjacentTransitions(clips,[dip])).toEqual([dip]);
  });
});

describe("safe per-cut dip duration control",()=>{
  it("caps by both neighboring clip lengths while leaving room for two frames",()=>{
    const short=[{id:"a",duration:0.3},{id:"b",duration:0.2}];
    const max=maxCutTransitionDuration(short,[],"a",30);
    expect(max).toBe(0.26);
    expect(validateCutTransitions(short,[{...dip,duration_seconds:max}],30)).toBe("");
    expect(validateCutTransitions(short,[{...dip,duration_seconds:max+0.01}],30)).toMatch(/too short/);
  });
  it("accounts for adjacent entering and exiting fade envelopes",()=>{
    const surrounding=[{id:"a",duration:1},{id:"b",duration:0.5},{id:"c",duration:1}];
    const next:CutTransition={from_clip_id:"b",to_clip_id:"c",kind:"dip_to_black",duration_seconds:0.5};
    const max=maxCutTransitionDuration(surrounding,[next],"a",30);
    expect(max).toBe(0.36);
    expect(validateCutTransitions(surrounding,[{...dip,duration_seconds:max},next],30)).toBe("");
  });
  it("rejects unavailable cuts, impossible short clips and unsupported frame clocks",()=>{
    expect(maxCutTransitionDuration(clips,[],"missing",30)).toBe(0);
    expect(maxCutTransitionDuration([{id:"a",duration:0.04},{id:"b",duration:2}],[],"a",30)).toBe(0);
    expect(maxCutTransitionDuration(clips,[],"c",30)).toBe(0);
    expect(maxCutTransitionDuration(clips,[],"a",25)).toBe(0);
  });
});
