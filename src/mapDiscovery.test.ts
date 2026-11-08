import {expect,it} from "vitest";
import {mapCaptureYears} from "./mapDiscovery";
it("derives years from matching GPS paths with one year read per matching item",()=>{
  const photos=Array.from({length:50000},(_,i)=>({path:String(i),year:2000+i%25})),located=photos.filter((_,i)=>i%2===0);let calls=0;
  expect(mapCaptureYears(photos,located,p=>{calls++;return p.year;})).toHaveLength(25);expect(calls).toBe(25000);
});
