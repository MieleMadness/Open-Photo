import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {SequenceTitles} from "./SequenceTitles";
import {type SequenceProject,type SequenceSession,validSequence} from "./videoSequence";
import {defaultEdits} from "./videoProject";
import {validTitleText} from "./titleText";

afterEach(cleanup);
function project(text="Your title"):SequenceProject{return {
  schema_version:3,id:"project",title:"Project",frame_rate:30,
  output:{width:96,height:64,fit:"fit"},
  assets:[{id:"photo",relative_path:"photo.png",bytes:100,sha256:"0".repeat(64),media:{kind:"photo",width:96,height:64}}],
  clips:[{id:"clip",asset_id:"photo",source_in:{ticks:"0",time_base:"1/1000000"},source_out:{ticks:"5000000",time_base:"1/1000000"},edits:defaultEdits()}],
  titles:[{id:"title",text,start_seconds:0,end_seconds:3,x_percent:50,y_percent:50}],
};}
function editor(){
  const commit=vi.fn();
  const session={history:{present:project()},commit} as unknown as SequenceSession;
  render(<SequenceTitles session={session} disabled={false} playhead={0}/>);
  const input=screen.getByRole("textbox",{name:"Title text"}) as HTMLInputElement;
  input.focus();
  return {input,commit};
}
it.each(["Your title","Full frame 01","Text \\ path: 50%","你好世界","😀".repeat(100)])("accepts native-compatible title text %s",text=>{
  expect(validTitleText(text)).toBe(true);
  expect(validSequence(project(text))).toBe("");
});
it.each([" ","a".repeat(101),"😀".repeat(101),"line\nbreak","\u0000","\u007f","\u0085","\u009f","\ud800"])("rejects invalid title text %j",text=>{
  expect(validTitleText(text)).toBe(false);
  expect(validSequence(project(text))).toBeTruthy();
});
it("commits ordinary title text on blur",()=>{
  const {input,commit}=editor();
  fireEvent.change(input,{target:{value:"Full frame 01"}});
  fireEvent.blur(input);
  expect(commit).toHaveBeenCalledWith(expect.objectContaining({titles:[expect.objectContaining({text:"Full frame 01"})]}));
});
it("Escape discards the edit without committing on blur and allows the next edit",()=>{
  const {input,commit}=editor();
  fireEvent.change(input,{target:{value:"Unwanted edit"}});
  fireEvent.keyDown(input,{key:"Escape"});
  expect(input.value).toBe("Your title");
  expect(commit).not.toHaveBeenCalled();
  input.focus();
  fireEvent.change(input,{target:{value:"Wanted edit"}});
  fireEvent.keyDown(input,{key:"Enter"});
  expect(commit).toHaveBeenCalledOnce();
  expect(commit).toHaveBeenCalledWith(expect.objectContaining({titles:[expect.objectContaining({text:"Wanted edit"})]}));
});
it("keeps an IME composition active when Enter selects a candidate",()=>{
  const {input,commit}=editor();
  fireEvent.change(input,{target:{value:"你好"}});
  fireEvent.keyDown(input,{key:"Enter",isComposing:true});
  expect(document.activeElement).toBe(input);
  expect(commit).not.toHaveBeenCalled();
  fireEvent.keyDown(input,{key:"Enter"});
  expect(commit).toHaveBeenCalledOnce();
});
it("preserves an invalid draft with accessible feedback until corrected or canceled",()=>{
  const {input,commit}=editor();
  fireEvent.change(input,{target:{value:"a".repeat(101)}});
  fireEvent.blur(input);
  expect(input.value).toBe("a".repeat(101));
  expect(input.getAttribute("aria-invalid")).toBe("true");
  expect(screen.getByRole("status").id).toBe(input.getAttribute("aria-describedby"));
  expect(commit).not.toHaveBeenCalled();
  input.focus();
  fireEvent.keyDown(input,{key:"Escape"});
  expect(input.value).toBe("Your title");
  expect(input.hasAttribute("aria-invalid")).toBe(false);
  expect(commit).not.toHaveBeenCalled();
});
