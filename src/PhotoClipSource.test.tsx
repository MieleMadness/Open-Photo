import {act,cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {PhotoClipSource} from "./PhotoClipSource";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
it("ignores an old photo result after selecting a different source",async()=>{
  let finish:(value:string)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(_command,args)=>(args as {path:string}).path==="old.png"?new Promise(resolve=>{finish=resolve}):"new-photo");
  const ui=render(<PhotoClipSource root="r" path="old.png"/>);ui.rerender(<PhotoClipSource root="r" path="new.png"/>);const image=await screen.findByAltText("Original timeline photo");await act(async()=>finish("old-photo"));expect(image.getAttribute("src")).toBe("new-photo");
});
