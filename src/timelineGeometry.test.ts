import {expect,it} from "vitest";
import {adjacentCutPosition,clipDropIndex,timelineTicks} from "./timelineGeometry";
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

it("seeks previous and next clip cuts including project edges",()=>{
  const cuts=[1.5,3,4.25];
  expect(adjacentCutPosition(2,cuts,"previous")).toBe(1.5);
  expect(adjacentCutPosition(2,cuts,"next")).toBe(3);
  expect(adjacentCutPosition(3,cuts,"previous")).toBe(1.5);
  expect(adjacentCutPosition(3,cuts,"next")).toBe(4.25);
  expect(adjacentCutPosition(0,cuts,"previous")).toBeNull();
  expect(adjacentCutPosition(0,cuts,"next")).toBe(1.5);
  expect(adjacentCutPosition(4.25,cuts,"next")).toBeNull();
  expect(adjacentCutPosition(4.25,cuts,"previous")).toBe(3);
});
it("avoids unstable seeks at boundaries and invalid projects",()=>{
  expect(adjacentCutPosition(1.5+1e-8,[1.5,3],"next")).toBe(3);
  expect(adjacentCutPosition(1.5-1e-8,[1.5,3],"previous")).toBe(0);
  expect(adjacentCutPosition(0,[],"next")).toBeNull();
  expect(adjacentCutPosition(NaN,[1],"next")).toBeNull();
  expect(adjacentCutPosition(1,[Infinity],"previous")).toBeNull();
});
