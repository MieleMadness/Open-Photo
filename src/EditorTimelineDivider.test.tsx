import {cleanup,createEvent,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {EditorTimelineDivider,timelineSplitBounds} from "./EditorTimelineDivider";
import {saveSetting} from "./userData";
vi.mock("./userData",()=>({saveSetting:vi.fn((key:string,value:string)=>localStorage.setItem(key,value))}));
beforeEach(()=>{localStorage.clear();vi.clearAllMocks();});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
function setup(){const host={current:document.createElement("section")},onHeight=vi.fn();Object.defineProperty(host.current,"clientHeight",{value:800,configurable:true});return {...render(<EditorTimelineDivider host={host} onHeight={onHeight}/>),host,onHeight};}
function pointer(node:HTMLElement,type:"pointerDown"|"pointerMove"|"pointerUp",clientY:number){const event=createEvent[type](node);Object.defineProperties(event,{clientY:{value:clientY},pointerId:{value:1},button:{value:0}});fireEvent(node,event);}
it("previews resizing, cancels drafts and saves only a completed drag",()=>{
  setup();const divider=screen.getByRole("separator");
  pointer(divider,"pointerDown",400);pointer(divider,"pointerMove",350);expect(divider.getAttribute("aria-valuenow")).toBe("310");expect(saveSetting).not.toHaveBeenCalled();
  fireEvent.keyDown(divider,{key:"Escape"});pointer(divider,"pointerUp",350);expect(divider.getAttribute("aria-valuenow")).toBe("260");expect(saveSetting).not.toHaveBeenCalled();
  pointer(divider,"pointerDown",400);pointer(divider,"pointerUp",350);expect(saveSetting).toHaveBeenCalledExactlyOnceWith("openphoto.editor.timelineRatio",String(310/800));
});
it("restores the preference and supports keyboard limits/reset without repeated boundary saves",()=>{
  localStorage.setItem("openphoto.editor.timelineRatio","0.5");setup();const divider=screen.getByRole("separator");expect(divider.getAttribute("aria-valuenow")).toBe("400");
  fireEvent.keyDown(divider,{key:"ArrowUp"});expect(divider.getAttribute("aria-valuenow")).toBe("420");
  fireEvent.keyDown(divider,{key:"End"});expect(divider.getAttribute("aria-valuenow")).toBe("550");const count=vi.mocked(saveSetting).mock.calls.length;fireEvent.keyDown(divider,{key:"End"});expect(saveSetting).toHaveBeenCalledTimes(count);
  fireEvent.doubleClick(divider);expect(divider.getAttribute("aria-valuenow")).toBe("260");expect(localStorage.getItem("openphoto.editor.timelineRatio")).toBe("0.32");
});
it("clamps resized windows and notice space while retaining a usable preview",()=>{
  expect(timelineSplitBounds(580,80)).toEqual({minimum:260,maximum:330,available:490});
  const notices=timelineSplitBounds(580,190);expect(notices.available-notices.maximum).toBe(160);
  localStorage.setItem("openphoto.editor.timelineRatio","bad");setup();expect(screen.getByRole("separator").getAttribute("aria-valuenow")).toBe("260");
});
