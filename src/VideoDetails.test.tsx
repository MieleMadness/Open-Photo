import {render,screen,waitFor,cleanup} from "@testing-library/react";
import {afterEach,it,expect,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {VideoDetails} from "./VideoDetails";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks()});
const info={duration_seconds:12.5,codec:"h264",width:1920,height:1080,frame_rate:"30000/1001",rotation_degrees:-90,hdr:false,color_transfer:null,audio:[],warnings:["This video has no audio track."]};
it("shows actual video properties and silent-video information",async()=>{
  vi.mocked(invoke).mockResolvedValue(info);
  render(<VideoDetails root="library" path="phone.mov"/>);
  await screen.findByText("1920 × 1080");expect(screen.getByText(/29.970 fps/)).toBeTruthy();expect(screen.getByText("No audio track")).toBeTruthy();
});
it("ignores an obsolete inspection when another clip is selected",async()=>{
  let finish:(value:unknown)=>void=()=>{};
  vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve}));
  const {rerender}=render(<VideoDetails root="library" path="old.mov"/>);
  vi.mocked(invoke).mockResolvedValue({...info,width:640,height:480});
  rerender(<VideoDetails root="library" path="new.mov"/>);
  await screen.findByText("640 × 480");finish(info);
  await waitFor(()=>expect(screen.queryByText("1920 × 1080")).toBeNull());
});
it("shows inspection failures without affecting video playback",async()=>{
  vi.mocked(invoke).mockRejectedValue(Error("Invalid video"));
  render(<VideoDetails root="library" path="broken.mov"/>);
  expect(await screen.findByRole("alert")).toBeTruthy();expect(screen.getByText("Retry video inspection")).toBeTruthy();
});
