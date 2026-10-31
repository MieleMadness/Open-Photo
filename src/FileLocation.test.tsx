import {cleanup,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
const invoke=vi.hoisted(()=>vi.fn());
vi.mock("@tauri-apps/api/core",()=>({invoke}));
vi.mock("./PhotoMap",()=>({default:({points}:{points:{lat:number}[]})=><div data-testid="map">{points[0].lat}</div>}));
import FileLocation from "./FileLocation";
const props={root:"C:/photos",path:"C:/photos/a.jpg",extension:"jpg",modified:1,desktop:true};
afterEach(()=>{cleanup();vi.resetAllMocks();});
it("reads only the selected file and accepts zero coordinates",async()=>{
  invoke.mockResolvedValue([{path:props.path,lat:0,lon:0}]);
  render(<FileLocation {...props}/>);
  expect(await screen.findByTestId("map")).toBeTruthy();
  expect(screen.getByText("0.00000, 0.00000")).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("gps_for_photos",{root:props.root,paths:[props.path]});
});
it("does not read unsupported media",()=>{
  render(<FileLocation {...props} extension="mov"/>);
  expect(screen.getByText(/Support for other formats/)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});
it("ignores a previous file's late response",async()=>{
  let finish!:(value:unknown)=>void;
  invoke.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve})).mockResolvedValueOnce([]);
  const {rerender}=render(<FileLocation {...props}/>);
  rerender(<FileLocation {...props} path="C:/photos/b.jpg"/>);
  await screen.findByText(/No GPS location/);
  finish([{path:props.path,lat:12,lon:34}]);
  await waitFor(()=>expect(screen.queryByTestId("map")).toBeNull());
});
