import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {LibraryFilmstrip} from "./LibraryFilmstrip";
afterEach(cleanup);
it("renders a bounded neighbor window and selects media without creating players",()=>{
 const items=Array.from({length:1000},(_,i)=>({path:String(i),name:`Image ${i}`})),select=vi.fn();
 const {container}=render(<LibraryFilmstrip items={items} current="0" onSelect={select} thumbnail={item=><img src="thumb" alt={item.name}/>}/>);
 expect(screen.getAllByRole("button").length).toBeLessThan(20);fireEvent.click(screen.getByRole("button",{name:"View Image 1"}));expect(select).toHaveBeenCalledWith(items[1]);expect(container.querySelector("video")).toBeNull();
});

it("wheel scrolls the filmstrip horizontally without selecting media",()=>{
 const items=Array.from({length:50},(_,i)=>({path:String(i),name:`Image ${i}`})),select=vi.fn();
 const {container}=render(<LibraryFilmstrip items={items} current="0" onSelect={select} thumbnail={item=><span>{item.name}</span>}/>);
 const scroll=container.querySelector(".viewer-filmstrip-scroll") as HTMLDivElement;
 Object.defineProperty(scroll,"scrollWidth",{value:5600,configurable:true});
 Object.defineProperty(scroll,"clientWidth",{value:800,configurable:true});
 fireEvent.wheel(scroll,{deltaY:120,deltaX:0});
 expect(scroll.scrollLeft).toBe(120);
 expect(select).not.toHaveBeenCalled();
});
