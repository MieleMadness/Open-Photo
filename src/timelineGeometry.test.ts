import {expect,it} from "vitest";
import {clipDropIndex,timelineTicks} from "./timelineGeometry";
it("keeps ruler labels readable and work bounded across zoom and duration",()=>{
  expect(timelineTicks(.5,800).map(tick=>tick.label)).toContain("0.1s");
  expect(timelineTicks(3661,1200).find(tick=>tick.seconds===3600)?.label).toBe("1:00:00");
  const zoomed=timelineTicks(3600,16000);expect(zoomed.length).toBeLessThanOrEqual(201);
  expect(zoomed.every((tick,i)=>tick.left>=0&&tick.left<=100&&(!i||tick.seconds>zoomed[i-1].seconds))).toBe(true);
  expect(timelineTicks(0,800)).toEqual([]);expect(timelineTicks(Infinity,800)).toEqual([]);
});
it("moves to either side of a target without shifting adjacent no-op drops",()=>{
  expect(clipDropIndex(0,2,true)).toBe(2);expect(clipDropIndex(2,0,false)).toBe(0);
  expect(clipDropIndex(0,1,false)).toBe(0);expect(clipDropIndex(1,1,true)).toBe(1);
});
