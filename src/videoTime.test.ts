import {expect,it} from "vitest";
import {clockTime,markSeconds,markTime} from "./videoTime";
it("retains source ticks across NTSC time bases and long selections",()=>{
  const mark=markTime(3600.123456,"1/90000");
  expect(mark).toEqual({ticks:"324011111",time_base:"1/90000"});
  expect(Math.abs(markSeconds(mark)-3600.123456)).toBeLessThan(1/90000);
  expect(markTime(1001/30000,"1001/30000").ticks).toBe("1");
});
it("uses integer microseconds when the source has no usable time base",()=>{
  expect(markTime(0.123456,null)).toEqual({ticks:"123456",time_base:"1/1000000"});
  expect(markTime(1,"0/0").time_base).toBe("1/1000000");
  expect(clockTime(3661.123)).toBe("01:01:01.123");
});
