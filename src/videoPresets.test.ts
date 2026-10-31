import {expect,it} from "vitest";
import {exportCanvas} from "./videoExport";
it("shows resolved dimensions without changing the project canvas or enlarging small projects",()=>{
  const canvas={width:2160,height:3840,fit:"fit" as const};expect(exportCanvas(canvas,"hd1080")).toEqual({width:1080,height:1920,fit:"fit"});expect(exportCanvas(canvas,"uhd4k")).toEqual(canvas);
  expect(exportCanvas({width:1280,height:720,fit:"fill"},"uhd4k")).toEqual({width:1280,height:720,fit:"fill"});expect(canvas.width).toBe(2160);
});
