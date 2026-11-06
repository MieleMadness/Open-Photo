import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {VideoExportPanel} from "./VideoExportPanel";
import {exportFilename,refreshVideoExport,startVideoExport,hasStartingVideoExport,setAppClosing,type ExportStatus} from "./videoExport";
import type {VideoProject} from "./videoProject";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
const status=():ExportStatus=>({id:"job",title:"My video",destination:"/output/new.mp4",active:true,can_cancel:true,stage:"encoding",fraction:0.45,bytes:1048576,elapsed_seconds:2,error:null,result:null});
beforeEach(()=>{(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__={};});
afterEach(async()=>{cleanup();setAppClosing(false);vi.mocked(invoke).mockResolvedValue(null);await refreshVideoExport();delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;vi.resetAllMocks();});
it("shows real export progress and cancels the current job",async()=>{
  let current=status();vi.mocked(invoke).mockImplementation(async(command)=>{if(command==="cancel_video_export")current={...current,stage:"cancelling",can_cancel:false};return command==="video_export_status"?current:undefined;});
  render(<VideoExportPanel/>);expect(await screen.findByText("Encoding MP4")).toBeTruthy();expect((screen.getByLabelText("Video export progress") as HTMLProgressElement).value).toBe(0.45);
  fireEvent.click(screen.getByText("Cancel video export"));await screen.findByText("Cancelling export");expect(invoke).toHaveBeenCalledWith("cancel_video_export",{id:"job"});expect((screen.getByText("Cancel video export") as HTMLButtonElement).disabled).toBe(true);
});
it("offers completed output only after publication and dismisses without deleting it",async()=>{
  const current={...status(),active:false,can_cancel:false,stage:"completed",fraction:1,result:{path:"/output/new.mp4",bytes:123,width:1920,height:1080,duration_seconds:3,warnings:[]}} as ExportStatus;
  let dismissed=false;vi.mocked(invoke).mockImplementation(async(command)=>{if(command==="dismiss_video_export")dismissed=true;return command==="video_export_status"&&!dismissed?current:null;});
  render(<VideoExportPanel/>);await screen.findByText("Export complete");fireEvent.click(screen.getByText("Open exported video"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("open_video_export",{id:"job"}));
  fireEvent.click(screen.getByText("Dismiss"));await waitFor(()=>expect(screen.queryByLabelText("Video export")).toBeNull());
});
it("does not offer a finished file when export fails",async()=>{
  vi.mocked(invoke).mockResolvedValue({...status(),active:false,stage:"failed",error:"Not enough free space",result:null});
  render(<VideoExportPanel/>);expect(await screen.findByText("Not enough free space")).toBeTruthy();expect(screen.queryByText("Open exported video")).toBeNull();
});
it("creates Windows-safe default names while retaining Unicode",()=>{expect(exportFilename("旅行: summer")).toBe("旅行_ summer.edited.mp4");expect(exportFilename("CON")).toBe("video-CON.edited.mp4");});
it("tracks pending native starts and rejects new starts once window closure begins",async()=>{
  let finish:(id:string)=>void=()=>{};vi.mocked(invoke).mockImplementation(async(command)=>{if(command==="start_video_export")return new Promise(resolve=>{finish=resolve});if(command==="video_export_status")return status();});
  const project={title:"Snapshot"} as VideoProject;const options={destination:"new.mp4",size:"project" as const,acknowledge_first_audio:false};
  const pending=startVideoExport("library",project,options);expect(hasStartingVideoExport()).toBe(true);setAppClosing(true);
  await expect(startVideoExport("library",project,options)).rejects.toThrow("app is closing");expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="start_video_export")).toHaveLength(1);
  finish("job");await pending;expect(hasStartingVideoExport()).toBe(false);
});

it("ignores delayed status replies from before a newly accepted export",async()=>{
  let finish:(value:ExportStatus|null)=>void=()=>{};
  let polls=0;
  vi.mocked(invoke).mockImplementation(async(command)=>{
    if(command==="start_video_export")return "new-job";
    if(command==="video_export_status"&&++polls===1)return new Promise(resolve=>{finish=resolve});
    return {...status(),id:"new-job"};
  });
  const stale=refreshVideoExport();
  await startVideoExport("library",{title:"New export"} as VideoProject,{destination:"new.mp4",size:"project",acknowledge_first_audio:false});
  finish(null);
  expect((await stale)?.id).toBe("new-job");
});
it("does not let an older response replace a completed export",async()=>{
  let finish:(value:ExportStatus)=>void=()=>{};let polls=0;
  const completed={...status(),active:false,stage:"completed"} as ExportStatus;
  vi.mocked(invoke).mockImplementation(async()=>++polls===1?new Promise(resolve=>{finish=resolve}):completed);
  const old=refreshVideoExport();await refreshVideoExport();finish(status());
  expect((await old)?.stage).toBe("completed");
});
