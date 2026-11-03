import {expect,it} from "vitest";
import {adjacentPlaybackFrame,playbackFrameRate} from "./frameSeek";

it("parses bounded rational and integer playback frame rates",()=>{
  expect(playbackFrameRate("30000/1001")).toBeCloseTo(29.97002997);
  expect(playbackFrameRate("60")).toBe(60);
  for(const value of [null,"","0/1","30/0","300/1","1/2","NaN","30/1/2","abc","99999999999999999/1"])expect(playbackFrameRate(value)).toBeNull();
});
it("steps adjacent nominal frame clock ticks and respects source duration",()=>{
  expect(adjacentPlaybackFrame(0,"30/1",1,2)).toBeCloseTo(1/30);
  expect(adjacentPlaybackFrame(1/30,"30/1",1,2)).toBeCloseTo(2/30);
  expect(adjacentPlaybackFrame(2/30,"30/1",-1,2)).toBeCloseTo(1/30);
  expect(adjacentPlaybackFrame(0,"30/1",-1,2)).toBeNull();
  expect(adjacentPlaybackFrame(2,"30/1",1,2)).toBeNull();
  expect(adjacentPlaybackFrame(0.99,"30/1",1,1)).toBe(1);
  expect(adjacentPlaybackFrame(0.15,"30000/1001",1,2)).toBeCloseTo(6*1001/30000);
});
it("rejects unbounded or unknown playback time instead of seeking unexpectedly",()=>{
  expect(adjacentPlaybackFrame(Infinity,"30/1",1,3)).toBeNull();
  expect(adjacentPlaybackFrame(1,"30/1",1,NaN)).toBeNull();
  expect(adjacentPlaybackFrame(1,null,1,3)).toBeNull();
});
