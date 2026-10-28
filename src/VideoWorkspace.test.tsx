import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {refreshVideoExport} from "./videoExport";
import {VideoWorkspace} from "./VideoWorkspace";
import {defaultEdits,discardProjectSaves,type LoadedProject} from "./videoProject";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(p:string)=>`asset:${p}`}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn(),save:vi.fn()}));
const info={duration_seconds:10,codec:"h264",width:96,height:64,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]};
const value=():LoadedProject=>({project:{schema_version:1,id:"p",title:"My edit",asset:{id:"a",relative_path:"source.mp4",sha256:"0".repeat(64),bytes:200,info},clips:[{id:"c",asset_id:"a",source_in:{ticks:"0",time_base:"1/90000"},source_out:{ticks:"900000",time_base:"1/90000"},edits:defaultEdits()}],output:{width:1920,height:1080,fit:"fit"}},revision:1,source_path:"/library/source.mp4",source_status:"ready"});
beforeEach(()=>{vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="recent_video_projects")return [{id:"p",title:"My edit"}];
  if(command==="inspect_video")return info;
  if(command==="create_video_project"||command==="load_video_project"||command==="relink_video_project")return value();
  if(command==="save_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
});});
afterEach(async()=>{cleanup();discardProjectSaves();vi.mocked(invoke).mockResolvedValue(null);await refreshVideoExport();vi.resetAllMocks();});
it("autosaves a transform, supports durable undo/redo, and reopens a recent draft",async()=>{
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_video_project",expect.objectContaining({project:expect.objectContaining({clips:[expect.objectContaining({edits:expect.objectContaining({quarter_turns:1})})]})})));
  fireEvent.click(screen.getByText("Undo video edit"));await waitFor(()=>expect((screen.getByLabelText("Video rotation") as HTMLSelectElement).value).toBe("0"));
  fireEvent.click(screen.getByText("Redo video edit"));await waitFor(()=>expect((screen.getByLabelText("Video rotation") as HTMLSelectElement).value).toBe("1"));
  await screen.findByText("All project changes saved locally.");fireEvent.change(screen.getByLabelText("Recent video projects"),{target:{value:"p"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("load_video_project",{root:"library",id:"p"}));
});
it("keeps a missing project editable and requires explicit source relinking",async()=>{
  vi.mocked(invoke).mockImplementation(async(command)=>{if(command==="recent_video_projects")return [{id:"p",title:"My edit"}];if(command==="load_video_project")return {...value(),source_path:null,source_status:"Source missing"};if(command==="relink_video_project")return {...value(),revision:2};});
  vi.mocked(open).mockResolvedValue("/library/moved.mp4");
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);
  await screen.findByLabelText("Recent video projects");fireEvent.change(screen.getByLabelText("Recent video projects"),{target:{value:"p"}});
  expect(await screen.findByText(/Source needs relinking/)).toBeTruthy();expect((screen.getByText("Preview edits") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByText("Relink original video…"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("relink_video_project",{root:"library",id:"p",path:"/library/moved.mp4",expectedRevision:1}));
});
it("ignores a rendered preview that arrives after an edit has invalidated it",async()=>{
  let finish:(value:unknown)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="recent_video_projects")return [];
    if(command==="inspect_video")return info;
    if(command==="create_video_project")return value();
    if(command==="start_video_project_preview")return "edit-job";
    if(command==="video_preview_status")return new Promise(resolve=>{finish=resolve});
    if(command==="save_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
  });
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Preview edits"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("video_preview_status",{id:"edit-job"}));
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});
  await act(async()=>finish({state:"complete",result:{source:"obsolete.mp4",warnings:[]},error:null}));
  expect(screen.queryByLabelText("Edited video")).toBeNull();expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"edit-job"});
});
it("renders the latest saved edit when controls change while preview waits for autosave",async()=>{
  let finish:(revision:number)=>void=()=>{};let saves=0;
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="recent_video_projects")return [];
    if(command==="inspect_video")return info;
    if(command==="create_video_project")return value();
    if(command==="save_video_project"){if(++saves===1)return new Promise(resolve=>{finish=resolve});return 3;}
    if(command==="start_video_project_preview")return "latest-job";
    if(command==="video_preview_status")return {state:"complete",result:{source:"latest.mp4",warnings:[]}};
  });
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});fireEvent.click(screen.getByText("Preview edits"));
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"2"}});await act(async()=>finish(2));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("start_video_project_preview",expect.objectContaining({project:expect.objectContaining({clips:[expect.objectContaining({edits:expect.objectContaining({quarter_turns:2})})]})})));
});
it("exports an immutable saved snapshot and leaves the native job running after editor closure",async()=>{
  let snapshot:unknown;
  vi.mocked(save).mockResolvedValue("/output/new.mp4");
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="recent_video_projects")return [];
    if(command==="inspect_video")return info;
    if(command==="create_video_project")return value();
    if(command==="save_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
    if(command==="start_video_export"){snapshot=structuredClone(args);return "export-job";}
    if(command==="video_export_status")return {id:"export-job",active:true,stage:"encoding"};
  });
  const view=render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});fireEvent.click(screen.getByText("Export video MP4…"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("start_video_export",expect.objectContaining({options:{destination:"/output/new.mp4",size:"project",acknowledge_first_audio:false}})));
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"2"}});expect(snapshot).toEqual(expect.objectContaining({project:expect.objectContaining({clips:[expect.objectContaining({edits:expect.objectContaining({quarter_turns:1})})]})}));
  view.unmount();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="cancel_video_export")).toBe(false);
});
it("does not start an export if the editor closes while its save dialog is pending",async()=>{
  let finish:(path:string)=>void=()=>{};vi.mocked(save).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
  const view=render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Export video MP4…"));
  await waitFor(()=>expect(save).toHaveBeenCalled());view.unmount();await act(async()=>finish("/output/late.mp4"));expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="start_video_export")).toBe(false);
});
it("requires an explicit first-track choice for multiple audio tracks unless audio is muted",async()=>{
  const multi=value();multi.project.asset.info.audio=[1,2].map(stream=>({stream,codec:"aac",sample_rate:"48000",channels:2,channel_layout:"stereo"}));
  vi.mocked(invoke).mockImplementation(async(command)=>{if(command==="recent_video_projects")return [];if(command==="inspect_video")return multi.project.asset.info;if(command==="create_video_project")return multi;if(command==="save_video_project")return 2;});
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");
  await waitFor(()=>expect((screen.getByText("New project from selected video") as HTMLButtonElement).disabled).toBe(false));
  const button=screen.getByText("Export video MP4…") as HTMLButtonElement;expect(button.disabled).toBe(true);fireEvent.click(screen.getByLabelText("Confirm first audio track only"));expect(button.disabled).toBe(false);
  fireEvent.click(screen.getByLabelText("Confirm first audio track only"));expect(button.disabled).toBe(true);fireEvent.click(screen.getByLabelText("Mute video audio"));expect(button.disabled).toBe(false);
});

it("autosaves 4K and source-resolution canvas choices without changing the source",async()=>{
  const loaded=value();loaded.project.asset.info={...info,width:3840,height:1080};
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="create_video_project")return loaded;
    if(command==="recent_video_projects")return [];
    if(command==="inspect_video")return loaded.project.asset.info;
    if(command==="save_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
  });
  render(<VideoWorkspace root="library" path="source.mp4" onOpenExternal={()=>{}}/>);
  fireEvent.click(screen.getByText("New project from selected video"));await screen.findByDisplayValue("My edit");
  fireEvent.change(screen.getByLabelText("Video canvas"),{target:{value:"3840x2160"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_video_project",expect.objectContaining({project:expect.objectContaining({output:{width:3840,height:2160,fit:"fit"}})})));
  fireEvent.change(screen.getByLabelText("Video canvas"),{target:{value:"3840x1080"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_video_project",expect.objectContaining({project:expect.objectContaining({output:{width:3840,height:1080,fit:"fit"}})})));
  expect(loaded.project.asset.info.width).toBe(3840);expect(loaded.project.output.width).toBe(1920);
});
