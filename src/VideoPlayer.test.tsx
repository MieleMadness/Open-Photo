import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {IPhoneMotion} from "./IPhoneMedia";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(path:string)=>`asset:${path}`}));
const info={duration_seconds:20,codec:"h264",width:96,height:64,frame_rate:"30/1",time_base:"1/90000",rotation_degrees:0,hdr:false,color_transfer:null,audio:[],warnings:[]};
let sequence=0;
beforeEach(()=>{
  sequence=0;
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="inspect_video")return info;
    if(command==="video_poster")return "data:image/png;base64,poster";
    if(command==="video_source")return "original.mp4";
    if(command==="start_video_preview")return `job-${++sequence}`;
    if(command==="video_preview_status")return {state:"complete",result:{source:"preview.mp4",proxy:sequence>1,info,filmstrip:[{seconds:5,image:"data:image/jpeg;base64,abc"}],warnings:[]},error:null};
    return undefined;
  });
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.resetAllMocks()});
it("loads a selected standalone video without another click and leaves audio playback to the controls",async()=>{
  render(<IPhoneMotion autoLoad root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  const video=await screen.findByLabelText("Selected video") as HTMLVideoElement;
  expect(video.src).toBe("asset:original.mp4");expect(video.autoplay).toBe(false);
  expect(screen.queryByText("Play Live Photo / video")).toBeNull();
  expect(invoke).toHaveBeenCalledWith("start_video_preview",{root:"library",path:"one.mp4",proxy:false});
  fireEvent.play(video);expect(video.autoplay).toBe(true);
});
it("shows a video poster before playback and keeps it on the player",async()=>{
  render(<IPhoneMotion root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  expect((await screen.findByAltText("Selected video preview") as HTMLImageElement).src).toBe("data:image/png;base64,poster");
  expect(invoke).toHaveBeenCalledWith("video_poster",{root:"library",path:"one.mp4",maxEdge:640,forPlayer:true});
  fireEvent.click(screen.getByLabelText("Play selected video"));
  const video=await screen.findByLabelText("Selected video") as HTMLVideoElement;
  expect(video.poster).toBe("data:image/png;base64,poster");expect(video.autoplay).toBe(true);
});
it("automatically switches unsupported originals to one compatible preview and reports a proxy failure",async()=>{
  const {video}=await open();video.currentTime=6;fireEvent.timeUpdate(video);
  Object.defineProperty(video,"error",{configurable:true,value:{code:4,message:"Unsupported source format"}});fireEvent.error(video);
  await screen.findByText(/Playing a smaller compatible preview/);
  expect(invoke).toHaveBeenCalledWith("start_video_preview",{root:"library",path:"one.mp4",proxy:true});
  const proxy=screen.getByLabelText("Selected video") as HTMLVideoElement;
  Object.defineProperty(proxy,"duration",{configurable:true,value:20});fireEvent.loadedMetadata(proxy);expect(proxy.currentTime).toBe(6);
  Object.defineProperty(proxy,"error",{configurable:true,value:{code:3,message:"Decoder failed"}});fireEvent.error(proxy);
  expect(await screen.findByText(/compatible preview could not play.*media error 3/)).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="start_video_preview")).toHaveLength(2);
});
it("reports streaming failures without starting a codec conversion",async()=>{
  const {video}=await open();Object.defineProperty(video,"error",{configurable:true,value:{code:2,message:"Stream read failed"}});fireEvent.error(video);
  expect(await screen.findByText(/original video could not play.*media error 2/)).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="start_video_preview")).toHaveLength(1);
});
it("uses compatibility when the browser loads audio without a video track",async()=>{
  const {video}=await open();
  Object.defineProperty(video,"videoWidth",{configurable:true,value:0});Object.defineProperty(video,"videoHeight",{configurable:true,value:0});
  fireEvent.loadedData(video);
  await screen.findByText(/Playing a smaller compatible preview/);
  expect(invoke).toHaveBeenCalledWith("start_video_preview",{root:"library",path:"one.mp4",proxy:true});
});
async function open(){
  const view=render(<IPhoneMotion root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("Play Live Photo / video"));
  const video=await screen.findByLabelText("Selected video") as HTMLVideoElement;
  Object.defineProperty(video,"duration",{configurable:true,value:20});fireEvent.loadedMetadata(video);
  await screen.findByRole("button",{name:"Seek to 00:00:05.000"});
  return {view,video};
}
it("seeks in five-second steps and keeps playback position when changing to a proxy",async()=>{
  const {video}=await open();
  fireEvent.change(screen.getByLabelText("Seek video"),{target:{value:"5"}});
  fireEvent.click(screen.getByLabelText("Forward 5 seconds"));
  expect((screen.getByLabelText("Seek video") as HTMLInputElement).value).toBe("10");
  fireEvent.click(screen.getByLabelText("Back 5 seconds"));
  expect((screen.getByLabelText("Seek video") as HTMLInputElement).value).toBe("5");
  fireEvent.click(screen.getByText("Create compatible preview"));
  await screen.findByText(/Playing a smaller compatible preview/);
  const proxy=screen.getByLabelText("Selected video") as HTMLVideoElement;
  Object.defineProperty(proxy,"duration",{configurable:true,value:20});fireEvent.loadedMetadata(proxy);
  expect(proxy.currentTime).toBe(5);
  expect(video).toBeTruthy();
});
it("limits keyboard shortcuts to the player and resets state when selecting another clip",async()=>{
  const {view}=await open();
  fireEvent.keyDown(screen.getByLabelText("Video playback"),{key:"ArrowRight"});
  expect((screen.getByLabelText("Seek video") as HTMLInputElement).value).toBe("5");
  fireEvent.keyDown(screen.getByLabelText("Video playback"),{key:"ArrowRight",shiftKey:true});
  expect((screen.getByLabelText("Seek video") as HTMLInputElement).value).toBe("15");
  view.rerender(<IPhoneMotion root="library" path="two.mp4" onOpenExternal={()=>{}}/>);
  expect(screen.queryByLabelText("Selected video")).toBeNull();
  expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"job-1"});
});
it("cancels a job returned after the player has closed",async()=>{
  let finish:(id:string)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async(command)=>{
    if(command==="inspect_video")return info;
    if(command==="video_source")return "original.mp4";
    if(command==="start_video_preview")return new Promise<string>(resolve=>{finish=resolve});
  });
  const view=render(<IPhoneMotion root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("Play Live Photo / video"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("start_video_preview",{root:"library",path:"one.mp4",proxy:false}));
  view.unmount();await act(async()=>finish("late-job"));
  expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"late-job"});
});
it("keeps original playback available when derivative generation fails",async()=>{
  vi.mocked(invoke).mockImplementation(async(command)=>{
    if(command==="inspect_video")return info;
    if(command==="video_source")return "original.mp4";
    if(command==="start_video_preview")return "failed-job";
    if(command==="video_preview_status")return {state:"failed",error:"Disk full",result:null};
  });
  render(<IPhoneMotion root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("Play Live Photo / video"));
  expect(await screen.findByText(/Disk full/)).toBeTruthy();
  expect((screen.getByLabelText("Selected video") as HTMLVideoElement).src).toBe("asset:original.mp4");
});
it("does not expose standalone trim controls without an editing project",async()=>{
  await open();
  expect(screen.queryByText("Set trim start")).toBeNull();
  expect(screen.queryByText("Set trim end")).toBeNull();
  expect(screen.queryByText("Preview trim")).toBeNull();
});
it("cancels active generation and ignores a late completed status",async()=>{
  let finish:(value:unknown)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async(command)=>{
    if(command==="inspect_video")return info;
    if(command==="video_source")return "original.mp4";
    if(command==="start_video_preview")return "active-job";
    if(command==="video_preview_status")return new Promise(resolve=>{finish=resolve});
  });
  render(<IPhoneMotion root="library" path="one.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("Play Live Photo / video"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("video_preview_status",{id:"active-job"}));
  fireEvent.click(screen.getByText("Cancel"));
  await act(async()=>finish({state:"complete",result:{proxy:true,source:"obsolete.mp4",filmstrip:[],warnings:[],info},error:null}));
  expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"active-job"});
  expect((screen.getByLabelText("Selected video") as HTMLVideoElement).src).toBe("asset:original.mp4");
});

it("removes the typed timestamp form in favor of scrubber and skip controls",async()=>{
  await open();
  expect(screen.queryByLabelText("Video seek time")).toBeNull();
  expect(screen.queryByRole("button",{name:"Go to time"})).toBeNull();
  expect(screen.getByLabelText("Back 5 seconds")).toBeTruthy();
  expect(screen.getByLabelText("Forward 5 seconds")).toBeTruthy();
});
