import {cleanup,createEvent,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {TimelineTrimHandles,trimRange} from "./TimelineTrimHandles";
afterEach(cleanup);
function pointer(node:HTMLElement,type:"pointerDown"|"pointerMove"|"pointerUp"|"pointerCancel",x:number){
  const event=createEvent[type](node);Object.defineProperties(event,{clientX:{value:x},pointerId:{value:1},button:{value:0}});fireEvent(node,event);
}
function setup(){const props={range:{start:.125,end:2.125},limit:3,minimum:.0401,step:1/30,selected:true,disabled:false,token:{},index:0,pixelsPerSecond:()=>100,onCommit:vi.fn()};return {...render(<TimelineTrimHandles {...props}/>),props};}
it("previews a pointer trim without saves and commits once on release",()=>{
  const {props}=setup(),end=screen.getByRole("slider",{name:"Trim end of clip 1"});
  pointer(end,"pointerDown",200);pointer(end,"pointerMove",150);expect(props.onCommit).not.toHaveBeenCalled();
  expect(Number(end.getAttribute("aria-valuenow"))).toBeCloseTo(1.625);pointer(end,"pointerUp",150);
  expect(props.onCommit).toHaveBeenCalledExactlyOnceWith({start:.125,end:1.625});expect(screen.queryByRole("status")).toBeNull();
});
it("discards Escape, cancelled and stale gestures without saving",()=>{
  const {props,rerender}=setup(),end=screen.getByRole("slider",{name:"Trim end of clip 1"});
  pointer(end,"pointerDown",200);pointer(end,"pointerMove",150);fireEvent.keyDown(end,{key:"Escape"});pointer(end,"pointerUp",150);
  pointer(end,"pointerDown",200);pointer(end,"pointerMove",150);pointer(end,"pointerCancel",150);pointer(end,"pointerUp",150);
  pointer(end,"pointerDown",200);pointer(end,"pointerMove",150);rerender(<TimelineTrimHandles {...props} token={{}}/>);pointer(end,"pointerUp",150);
  pointer(end,"pointerDown",200);pointer(end,"pointerMove",150);fireEvent(window,new Event("blur"));pointer(end,"pointerUp",150);
  expect(props.onCommit).not.toHaveBeenCalled();
});
it("supports keyboard nudges and clamps minimum length/source boundaries",()=>{
  const {props}=setup(),start=screen.getByRole("slider",{name:"Trim start of clip 1"});
  fireEvent.keyDown(start,{key:"ArrowRight",shiftKey:true});expect(props.onCommit.mock.calls[0][0].start).toBeCloseTo(.125+10/30);
  expect(trimRange(props.range,"start",100,1/30,3,.0401).end-trimRange(props.range,"start",100,1/30,3,.0401).start).toBeCloseTo(.0401);
  expect(trimRange(props.range,"end",100,1/30,3,.0401).end).toBe(3);expect(trimRange(props.range,"start",-100,1/30,3,.0401).start).toBe(0);
});
