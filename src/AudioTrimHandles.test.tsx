import {cleanup,createEvent,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import type {AudioTrack} from "./videoSequence";
import {AudioTrimHandles} from "./AudioTrimHandles";

afterEach(cleanup);
const track:AudioTrack={id:"sound",asset_id:"song",start_seconds:5,source_in_seconds:2,duration_seconds:6,gain_percent:80,fade_in_seconds:1,fade_out_seconds:2};
function pointer(node:HTMLElement,type:"pointerDown"|"pointerMove"|"pointerUp"|"pointerCancel",x:number){
  const event=createEvent[type](node);
  Object.defineProperties(event,{clientX:{value:x},pointerId:{value:1},button:{value:0}});
  fireEvent(node,event);
}
function setup(){
  const commit=vi.fn();
  const view=render(<div className="sequence-audio-lane"><AudioTrimHandles track={track} trackNumber={2} sourceDuration={20} projectDuration={15} disabled={false} onCommit={commit}/></div>);
  const lane=view.container.querySelector(".sequence-audio-lane")!;
  Object.defineProperty(lane,"getBoundingClientRect",{value:()=>({width:1500,height:30,left:0,right:1500,top:0,bottom:30})});
  return {...view,commit};
}
it("drafts on pointer move and saves one edit on pointer release",()=>{
  const {commit}=setup();
  const start=screen.getByRole("slider",{name:"Trim start of audio track 2"});
  pointer(start,"pointerDown",100);
  pointer(start,"pointerMove",200);
  expect(commit).not.toHaveBeenCalled();
  expect(start.getAttribute("aria-valuenow")).toBe("6");
  expect((document.querySelector(".sequence-audio-bar") as HTMLElement).style.left).toBe("40%");
  expect((document.querySelector(".sequence-audio-bar") as HTMLElement).style.width).toBe(`${5/15*100}%`);
  pointer(start,"pointerUp",200);
  expect(commit).toHaveBeenCalledExactlyOnceWith({...track,start_seconds:6,source_in_seconds:3,duration_seconds:5});
  expect(screen.queryByRole("status")).toBeNull();
});
it("cancels Escape and pointer cancellation without persisting changes",()=>{
  const {commit}=setup();
  const end=screen.getByRole("slider",{name:"Trim end of audio track 2"});
  pointer(end,"pointerDown",100);pointer(end,"pointerMove",150);
  fireEvent.keyDown(end,{key:"Escape"});pointer(end,"pointerUp",150);
  pointer(end,"pointerDown",100);pointer(end,"pointerMove",150);
  pointer(end,"pointerCancel",150);pointer(end,"pointerUp",150);
  expect(commit).not.toHaveBeenCalled();
});
it("supports keyboard nudges and rejects disabled interaction",()=>{
  const {commit,rerender}=setup();
  const end=screen.getByRole("slider",{name:"Trim end of audio track 2"});
  fireEvent.keyDown(end,{key:"ArrowLeft",shiftKey:true});
  expect(commit).toHaveBeenCalledWith({...track,duration_seconds:5.9});
  rerender(<div className="sequence-audio-lane"><AudioTrimHandles track={track} trackNumber={2} sourceDuration={20} projectDuration={15} disabled onCommit={commit}/></div>);
  expect((screen.getByRole("slider",{name:"Trim end of audio track 2"}) as HTMLButtonElement).disabled).toBe(true);
});
