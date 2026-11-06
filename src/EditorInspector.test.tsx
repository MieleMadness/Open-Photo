import {useState} from "react";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {EditorInspector,type InspectorTab} from "./EditorInspector";
afterEach(cleanup);
function Example(){const [tab,onTab]=useState<InspectorTab>("clip");return <EditorInspector open tab={tab} onTab={onTab} onClose={()=>{}} clip={<input aria-label="Draft trim" defaultValue="1"/>} canvas={<p>Export settings</p>}/>;}
it("preserves drafts when switching inspector tasks",()=>{
  render(<Example/>);const draft=screen.getByRole("textbox",{name:"Draft trim"});fireEvent.change(draft,{target:{value:"999"}});
  fireEvent.click(screen.getByRole("tab",{name:"Canvas & export"}));expect(screen.queryByRole("textbox",{name:"Draft trim"})).toBeNull();
  fireEvent.click(screen.getByRole("tab",{name:"Clip"}));expect(screen.getByRole("textbox",{name:"Draft trim"})).toBe(draft);expect((draft as HTMLInputElement).value).toBe("999");
});
it("supports keyboard tab navigation and linked panels",()=>{
  render(<Example/>);const clip=screen.getByRole("tab",{name:"Clip"});clip.focus();fireEvent.keyDown(clip,{key:"ArrowRight"});
  const canvas=screen.getByRole("tab",{name:"Canvas & export"});expect(document.activeElement).toBe(canvas);expect(canvas.getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("tabpanel").id).toBe(canvas.getAttribute("aria-controls"));fireEvent.keyDown(canvas,{key:"Home"});expect(document.activeElement).toBe(clip);
});
