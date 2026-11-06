import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {LibraryFilmstrip} from "./LibraryFilmstrip";
afterEach(cleanup);
it("renders a bounded neighbor window and selects media without creating players",()=>{
 const items=Array.from({length:1000},(_,i)=>({path:String(i),name:`Image ${i}`})),select=vi.fn();
 const {container}=render(<LibraryFilmstrip items={items} current="0" onSelect={select} thumbnail={item=><img src="thumb" alt={item.name}/>}/>);
 expect(screen.getAllByRole("button").length).toBeLessThan(20);fireEvent.click(screen.getByRole("button",{name:"View Image 1"}));expect(select).toHaveBeenCalledWith(items[1]);expect(container.querySelector("video")).toBeNull();
});
