import {expect,it} from "vitest";
import {socialCanvases,isValidSocialCanvas} from "./socialCanvases";
it("provides exact, unique even-dimension social output presets",()=>{
  const keys=socialCanvases.map(item=>`${item.width}x${item.height}`);
  expect(new Set(keys).size).toBe(keys.length);
  expect(socialCanvases.every(item=>item.width>0&&item.height>0&&item.width%2===0&&item.height%2===0)).toBe(true);
  expect(isValidSocialCanvas(1080,1920)).toBe(true);
  expect(isValidSocialCanvas(1080,1350)).toBe(true);
  expect(isValidSocialCanvas(123,456)).toBe(false);
});
