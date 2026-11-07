import {expect,it} from "vitest";
import {previewDimensions} from "./previewDimensions";
it("rejects oversized decoded dimensions before RGBA allocation and bounds thumbnail output",()=>{
  for(const [w,h] of [[20001,1],[10000,5001],[0,1],[NaN,20]])expect(()=>previewDimensions(w,h,320)).toThrow();
  expect(previewDimensions(10000,5000,320)).toEqual({width:320,height:160});
  expect(previewDimensions(40,20,1600)).toEqual({width:40,height:20});
});
