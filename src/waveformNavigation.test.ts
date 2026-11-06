import {describe,expect,it} from "vitest";
import {waveformPosition,waveformSeek,waveformWindow} from "./waveformNavigation";

describe("waveform playhead navigation",()=>{
  it("uses the visible excerpt without claiming to draw the whole long source",()=>{
    expect(waveformWindow(240)).toBe(120);
    expect(waveformWindow(6.5)).toBe(6.5);
    expect(waveformWindow(-1)).toBe(0);
  });
  it("translates an excerpt position to global project seconds",()=>{
    expect(waveformSeek(12.25,1.5,8)).toBe(13.75);
    expect(waveformSeek(12.25,400,250)).toBe(132.25);
    expect(waveformSeek(12.25,-1,250)).toBe(12.25);
    expect(waveformSeek(-1,1,8)).toBeNull();
    expect(waveformSeek(1,Number.NaN,8)).toBeNull();
  });
  it("keeps the slider in range when the playhead is before or after the clip",()=>{
    expect(waveformPosition(3,10,20)).toBe(0);
    expect(waveformPosition(22,10,20)).toBe(12);
    expect(waveformPosition(900,10,200)).toBe(120);
    expect(waveformPosition(Number.NaN,10,20)).toBe(0);
  });
});
