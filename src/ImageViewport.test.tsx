import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {ImageViewport,boundImageView} from "./ImageViewport";
afterEach(cleanup);
it("bounds panning against the fitted image rather than the outside stage",()=>{
 expect(boundImageView({scale:2,x:900,y:-900},400,300,800,400)).toEqual({scale:2,x:200,y:-50});
 expect(boundImageView({scale:1,x:40,y:40},400,300,800,400)).toEqual({scale:1,x:0,y:0});
});
it("zooms only inside the stage, resets on another media identity, and leaves outward scrolling at fit",()=>{
 const {rerender}=render(<ImageViewport identity="one"><img src="one" alt="One"/></ImageViewport>);
 const stage=screen.getByLabelText("Zoomable image");const outward=new WheelEvent("wheel",{deltaY:30,bubbles:true,cancelable:true});stage.dispatchEvent(outward);expect(outward.defaultPrevented).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Zoom in image"}));expect(screen.getByLabelText("Image zoom").textContent).toContain("125%");
 fireEvent.keyDown(stage,{key:"0"});expect(screen.getByLabelText("Image zoom").textContent).toContain("100%");
 fireEvent.click(screen.getByRole("button",{name:"Zoom in image"}));rerender(<ImageViewport identity="two"><img src="two" alt="Two"/></ImageViewport>);expect(screen.getByLabelText("Image zoom").textContent).toContain("100%");
});
