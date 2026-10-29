import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {VideoSequences} from "./VideoSequences";
import {SequenceSession,type LoadedSequence} from "./videoSequence";
import {defaultEdits,discardProjectSaves,flushProjects,hasUnsavedProjects} from "./videoProject";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn(),save:vi.fn()}));
vi.mock("./VideoPlayer",()=>({VideoPlayer:({path}:{path:string})=><p>Preview: {path}</p>}));
const info={duration_seconds:2,codec:"h264",width:96,height:64,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]};
function loaded():LoadedSequence{return {project:{schema_version:2,id:"p",title:"Two short videos",assets:["one","two"].map(id=>({id,relative_path:`${id}.mp4`,sha256:(id==="one"?"1":"2").repeat(64),bytes:100,info})),clips:["one","two"].map(id=>({id:`clip-${id}`,asset_id:id,source_in:{ticks:"0",time_base:"1/90000"},source_out:{ticks:"180000",time_base:"1/90000"},edits:defaultEdits()})),output:{width:1920,height:1080,fit:"fit"},frame_rate:30},revision:1,sources:["one","two"].map(id=>({asset_id:id,path:`/library/${id}.mp4`,status:"ready"}))};}
beforeEach(()=>{vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="recent_video_sequences")return [{id:"p",title:"Two short videos"}];
  if(command==="create_video_sequence"||command==="load_video_sequence"||command==="import_video_sequence")return loaded();
  if(command==="add_sequence_videos")return {...loaded(),revision:2};
  if(command==="save_video_sequence")return (args as {expectedRevision:number}).expectedRevision+1;
});});
afterEach(()=>{cleanup();discardProjectSaves();vi.resetAllMocks();});
it("imports several short videos, previews a selected source, and autosaves the project name",async()=>{
  vi.mocked(open).mockResolvedValue(["/library/one.mp4","/library/two.mp4"]);
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);fireEvent.click(screen.getByText("New video project…"));
  await screen.findByDisplayValue("Two short videos");expect(invoke).toHaveBeenCalledWith("create_video_sequence",{root:"/library",paths:["/library/one.mp4","/library/two.mp4"]});
  fireEvent.click(within(screen.getByRole("region",{name:"Video media bin"})).getByRole("button",{name:/two.mp4/}));expect(screen.getByText("Preview: /library/two.mp4")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Multi-video project name"),{target:{value:"Saved assembly"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_video_sequence",expect.objectContaining({root:"/library",expectedRevision:1,project:expect.objectContaining({title:"Saved assembly"})})));
});
it("splits, reorders, trims and undoes timeline edits through revision-safe autosave",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved multi-video projects");
  fireEvent.change(screen.getByLabelText("Saved multi-video projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.change(screen.getByRole("slider",{name:"Project playhead"}),{target:{value:"1"}});
  fireEvent.click(screen.getByRole("button",{name:"Split at playhead"}));
  expect(screen.getByRole("button",{name:"Clip 3: two.mp4"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Move later"}));
  expect(screen.getByRole("button",{name:"Clip 2: one.mp4"}).getAttribute("aria-pressed")).toBe("true");
  const start=screen.getByRole("spinbutton",{name:"Clip start in source (seconds)"});fireEvent.change(start,{target:{value:"0.25"}});fireEvent.blur(start);
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.750");
  fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.000");
  fireEvent.click(screen.getByRole("button",{name:"Undo timeline edit"}));
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.750");
  await act(async()=>flushProjects());
  const calls=vi.mocked(invoke).mock.calls.filter(([name])=>name==="save_video_sequence");
  const last=calls.at(-1)![1] as {project:LoadedSequence["project"];expectedRevision:number};
  expect(last.project.clips).toHaveLength(3);expect(last.project.clips[1].source_in.ticks).toBe("22500");
  expect(last.expectedRevision).toBeGreaterThan(1);
});
it("keeps source assets after deleting every clip and can append the same video again",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved multi-video projects");
  fireEvent.change(screen.getByLabelText("Saved multi-video projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));
  expect(screen.getByText(/The timeline is empty/)).toBeTruthy();expect(screen.getByText(/2 \/ 50 videos/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Append selected video"}));expect(screen.getByRole("button",{name:"Clip 1: one.mp4"})).toBeTruthy();
  await act(async()=>flushProjects());
});
it("uses the verified backend result when duplicate imports are skipped",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved multi-video projects");fireEvent.change(screen.getByLabelText("Saved multi-video projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  vi.mocked(open).mockResolvedValue(["/library/one.mp4"]);fireEvent.click(screen.getByText("Add videos…"));
  await screen.findByText(/0 videos added/);expect(screen.getByText(/2 \/ 50 videos/)).toBeTruthy();
});
it("ignores a late project load after switching to another folder",async()=>{
  let finish:(value:LoadedSequence)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async command=>{if(command==="recent_video_sequences")return [{id:"p",title:"Saved"}];if(command==="load_video_sequence")return new Promise(resolve=>{finish=resolve;});});
  const ui=render(<VideoSequences roots={["/old"]} workspaceId="w"/>);await screen.findByLabelText("Saved multi-video projects");fireEvent.change(screen.getByLabelText("Saved multi-video projects"),{target:{value:"p"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("load_video_sequence",{root:"/old",id:"p"}));ui.rerender(<VideoSequences roots={["/new"]} workspaceId="w"/>);
  await act(async()=>finish(loaded()));expect(screen.queryByDisplayValue("Two short videos")).toBeNull();
});
it("protects new multi-video saves through the existing app-close save registry after unmount",async()=>{
  let finish:(revision:number)=>void=()=>{};vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const value=loaded(),session=new SequenceSession("/library",value);session.commit({...value.project,title:"Pending save"});session.release();expect(hasUnsavedProjects()).toBe(true);
  const flushed=flushProjects();finish(2);await flushed;expect(hasUnsavedProjects()).toBe(false);expect(session.revision).toBe(2);
});

it("can save an unsaved project copy after the draft save fails",async()=>{
  vi.mocked(invoke).mockImplementation(async command=>{if(command==="recent_video_sequences")return [{id:"p",title:"Saved"}];if(command==="load_video_sequence")return loaded();if(command==="save_video_sequence")throw Error("Disk full");});
  vi.mocked(save).mockResolvedValue("/backup/unsaved.json");render(<VideoSequences roots={["/library"]} workspaceId="w"/>);
  await screen.findByLabelText("Saved multi-video projects");fireEvent.change(screen.getByLabelText("Saved multi-video projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.change(screen.getByLabelText("Multi-video project name"),{target:{value:"Keep this edit"}});await screen.findByText(/Disk full/);
  fireEvent.click(screen.getByText("Save unsaved project copy…"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("write_video_sequence_copy",expect.objectContaining({destination:"/backup/unsaved.json",project:expect.objectContaining({title:"Keep this edit"})})));
});
