import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {LocalAiSettings} from "./LocalAiSettings";
afterEach(cleanup);
it("offers one switch for a verified installed bundle and exposes repair instead of downloads when incomplete",()=>{
  const onChange=vi.fn();const props={enabled:false,onChange,busy:false,message:"",onRefresh:vi.fn(),onCheck:vi.fn(),checking:false,runtime:null,device:"auto" as const,onDeviceChange:vi.fn()};
  const models=Array.from({length:7},(_,i)=>({kind:String(i),installed:true,compatible:true,bytes:10}));
  const view=render(<LocalAiSettings {...props} models={models}/>);fireEvent.click(screen.getByRole("switch",{name:"Local AI"}));expect(onChange).toHaveBeenCalledWith(true);
  expect(screen.getByText(/No download is needed/)).toBeTruthy();expect(screen.queryByText("Import ONNX")).toBeNull();
  view.rerender(<LocalAiSettings {...props} models={models.slice(1)}/>);expect((screen.getByRole("switch",{name:"Local AI"}) as HTMLInputElement).disabled).toBe(true);expect(screen.getByText(/Repair or reinstall/)).toBeTruthy();
});
