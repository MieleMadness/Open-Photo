import {describe,expect,it} from "vitest";
import {virtualRows} from "./virtualRows";

describe("Library virtual row windows",()=>{
  it("keeps the top and bottom bounded for 10,000 media rows",()=>{
    expect(virtualRows(10000,64,0,580)).toEqual({start:0,end:18});
    const bottom=virtualRows(10000,64,9999*64,580);
    expect(bottom.end).toBe(10000);
    expect(bottom.start).toBeGreaterThan(9900);
    expect(bottom.end-bottom.start).toBeLessThan(30);
  });
  it("adapts to small and tall viewports",()=>{
    const small=virtualRows(10000,64,640,250);
    const tall=virtualRows(10000,64,640,1200);
    expect(small.start).toBe(tall.start);
    expect(tall.end).toBeGreaterThan(small.end);
  });
  it("handles zero rows and negative offsets",()=>{
    expect(virtualRows(0,64,0,700)).toEqual({start:0,end:0});
    expect(virtualRows(15,64,-500,580).start).toBe(0);
  });
});
