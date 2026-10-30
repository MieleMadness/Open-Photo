import {act,cleanup,createEvent,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {VideoSequences} from "./VideoSequences";
import type {VideoSelection} from "./VideoPlayer";
import {SequenceSession,type LoadedSequence} from "./videoSequence";
import {defaultEdits,discardProjectSaves,flushProjects,hasUnsavedProjects} from "./videoProject";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn(),save:vi.fn()}));
vi.mock("./VideoPlayer",()=>({VideoPlayer:({path,root,selection}:{path:string;root:string;selection?:VideoSelection})=><div><p data-root={root}>Preview: {path}</p>{selection&&<button disabled={selection.disabled} onClick={()=>selection.onChange({ticks:"22500",time_base:"1/90000"},{ticks:"135000",time_base:"1/90000"})}>Use player trim marks</button>}</div>}));
const info={duration_seconds:2,codec:"h264",width:96,height:64,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]};
function loaded():LoadedSequence{return {project:{schema_version:2,id:"p",title:"Two short videos",assets:["one","two"].map(id=>({id,relative_path:`${id}.mp4`,sha256:(id==="one"?"1":"2").repeat(64),bytes:100,info})),clips:["one","two"].map(id=>({id:`clip-${id}`,asset_id:id,source_in:{ticks:"0",time_base:"1/90000"},source_out:{ticks:"180000",time_base:"1/90000"},edits:defaultEdits()})),output:{width:1920,height:1080,fit:"fit"},frame_rate:30},revision:1,sources:["one","two"].map(id=>({asset_id:id,path:`/library/${id}.mp4`,root:"/library",status:"ready"}))};}
it("searches the media bin and toggles panels without resetting the timeline or clip drafts",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w" initial={loaded()}/>);
  await screen.findByDisplayValue("Two short videos");
  fireEvent.change(screen.getByLabelText("Search project media"),{target:{value:"two"}});
  expect(within(screen.getByRole("region",{name:"Video media bin"})).queryByRole("button",{name:/one.mp4/})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Show media panel"}));expect(screen.queryByRole("region",{name:"Video media bin"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Show media panel"}));expect((screen.getByLabelText("Search project media") as HTMLInputElement).value).toBe("two");
  fireEvent.click(screen.getByRole("button",{name:"Clip 2: two.mp4"}));
  const volume=screen.getByLabelText("Audio volume (%)");fireEvent.change(volume,{target:{value:"999"}});fireEvent.blur(volume);
  fireEvent.click(screen.getByRole("button",{name:"Show clip controls"}));expect(screen.queryByRole("complementary",{name:"Clip inspector"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Export project MP4…"}));
  await screen.findByText(/Correct the highlighted field/);expect(screen.getByRole("complementary",{name:"Clip inspector"})).toBeTruthy();
  expect((screen.getByLabelText("Audio volume (%)") as HTMLInputElement).value).toBe("999");expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("button",{name:"Clip 2: two.mp4"}).getAttribute("aria-pressed")).toBe("true");
});
beforeEach(()=>{vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="workspace_video_projects")return {projects:[{id:"p",title:"Two short videos"}],warnings:[]};
  if(command==="edit_workspace_video_project")return (args as {action:{type:string}}).action.type==="add"?{...loaded(),revision:2}:loaded();
  if(command==="save_workspace_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
});});
afterEach(async()=>{cleanup();await act(async()=>{});discardProjectSaves();vi.resetAllMocks();});
it("imports several short videos, previews a selected source, and autosaves the project name",async()=>{
  vi.mocked(open).mockResolvedValue(["/library/one.mp4","/library/two.mp4"]);
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);fireEvent.click(screen.getByText("New video project…"));
  await screen.findByDisplayValue("Two short videos");expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"w",action:{type:"create",paths:["/library/one.mp4","/library/two.mp4"]}});
  fireEvent.click(within(screen.getByRole("region",{name:"Video media bin"})).getByRole("button",{name:/two.mp4/}));expect(screen.getByText("Preview: /library/two.mp4")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Video project name"),{target:{value:"Saved assembly"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({workspaceId:"w",expectedRevision:1,project:expect.objectContaining({title:"Saved assembly"})})));
});
it("fills the timeline proportionally even when the whole edit is shorter than one second",async()=>{
  const value=loaded();value.project.clips[0].source_out={ticks:"18000",time_base:"1/90000"};value.project.clips[1].source_out={ticks:"27000",time_base:"1/90000"};
  render(<VideoSequences roots={["/library"]} workspaceId="w" initial={value}/>);const first=await screen.findByRole("button",{name:"Clip 1: one.mp4"}),second=screen.getByRole("button",{name:"Clip 2: two.mp4"});
  expect(first.parentElement!.style.flexGrow).toBe("0.4");expect(second.parentElement!.style.flexGrow).toBe("0.6");
});
it("drops after the last clip, ignores adjacent no-op drops and undoes one reorder",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w" initial={loaded()}/>);
  const first=await screen.findByRole("button",{name:"Clip 1: one.mp4"});
  const target=screen.getByRole("button",{name:"Clip 2: two.mp4"}).parentElement!;
  vi.spyOn(target,"getBoundingClientRect").mockReturnValue({left:100,width:100} as DOMRect);
  const dataTransfer={setData:vi.fn(),effectAllowed:""};
  const drag=(type:"dragOver"|"drop",clientX:number)=>{const event=createEvent[type](target);Object.defineProperty(event,"clientX",{value:clientX});fireEvent(target,event);};
  fireEvent.dragStart(first,{dataTransfer});drag("drop",110);
  expect((screen.getByRole("button",{name:"Undo timeline edit"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.dragStart(first,{dataTransfer});drag("dragOver",190);
  expect(target.getAttribute("data-drop")).toBe("after");drag("drop",190);
  expect(screen.getByRole("button",{name:"Clip 2: one.mp4"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Undo timeline edit"}));
  expect(screen.getByRole("button",{name:"Clip 1: one.mp4"})).toBeTruthy();
});
it("saves a handle trim through the shared history and undoes the complete gesture",async()=>{
  const {container}=render(<VideoSequences roots={["/library"]} workspaceId="w" initial={loaded()}/>);
  const end=await screen.findByRole("slider",{name:"Trim end of clip 1"});
  vi.spyOn(container.querySelector(".timeline-track")!,"getBoundingClientRect").mockReturnValue({width:400} as DOMRect);
  const pointer=(type:"pointerDown"|"pointerMove"|"pointerUp",clientX:number)=>{const event=createEvent[type](end);Object.defineProperties(event,{clientX:{value:clientX},pointerId:{value:1},button:{value:0}});fireEvent(end,event);};
  pointer("pointerDown",200);pointer("pointerMove",150);
  expect((screen.getByRole("button",{name:"Undo timeline edit"}) as HTMLButtonElement).disabled).toBe(true);
  pointer("pointerUp",150);expect((screen.getByLabelText("Clip end in source (seconds)") as HTMLInputElement).value).toBe("1.5");
  await act(async()=>flushProjects());expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({project:expect.objectContaining({clips:expect.arrayContaining([expect.objectContaining({id:"clip-one",source_out:{ticks:"135000",time_base:"1/90000"}})])})}));
  fireEvent.click(screen.getByRole("button",{name:"Undo timeline edit"}));expect((screen.getByLabelText("Clip end in source (seconds)") as HTMLInputElement).value).toBe("2");
});
it("splits, reorders, trims and undoes timeline edits through revision-safe autosave",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved editing projects");
  fireEvent.change(screen.getByLabelText("Saved editing projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.change(screen.getByRole("slider",{name:"Project playhead"}),{target:{value:"1"}});
  fireEvent.click(screen.getByRole("button",{name:"Split at playhead"}));
  expect(screen.getByRole("button",{name:"Clip 3: two.mp4"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Move later"}));
  expect(screen.getByRole("button",{name:"Clip 2: one.mp4"}).getAttribute("aria-pressed")).toBe("true");
  const start=screen.getByRole("spinbutton",{name:"Clip start in source (seconds)"});fireEvent.change(start,{target:{value:"0.25"}});fireEvent.blur(start);
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.767");
  fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.000");
  fireEvent.click(screen.getByRole("button",{name:"Undo timeline edit"}));
  expect(screen.getByLabelText("Project duration").textContent).toBe("00:00:03.767");
  await act(async()=>flushProjects());
  const calls=vi.mocked(invoke).mock.calls.filter(([name])=>name==="save_workspace_video_project");
  const last=calls.at(-1)![1] as {project:LoadedSequence["project"];expectedRevision:number};
  expect(last.project.clips).toHaveLength(3);expect(last.project.clips[1].source_in.ticks).toBe("22500");
  expect(last.expectedRevision).toBeGreaterThan(1);
});
it("keeps source assets after deleting every clip and can append the same video again",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved editing projects");
  fireEvent.change(screen.getByLabelText("Saved editing projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));fireEvent.click(screen.getByRole("button",{name:"Ripple delete clip"}));
  expect(screen.getByText(/The timeline is empty/)).toBeTruthy();expect(screen.getByText(/2 \/ 50 videos/)).toBeTruthy();
  fireEvent.click(within(screen.getByRole("region",{name:"Video media bin"})).getByRole("button",{name:/one.mp4/}));fireEvent.click(screen.getByRole("button",{name:"Append selected video"}));expect(screen.getByRole("button",{name:"Clip 1: one.mp4"})).toBeTruthy();
  await act(async()=>flushProjects());
});
it("uses the verified backend result when duplicate imports are skipped",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w"/>);await screen.findByLabelText("Saved editing projects");fireEvent.change(screen.getByLabelText("Saved editing projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  vi.mocked(open).mockResolvedValue(["/library/one.mp4"]);fireEvent.click(screen.getByText("Add videos…"));
  await screen.findByText(/0 videos added/);expect(screen.getByText(/2 \/ 50 videos/)).toBeTruthy();
});
it("saves player trim marks to the selected timeline instance",async()=>{
  render(<VideoSequences roots={["/library"]} workspaceId="w" initial={loaded()}/>);await screen.findByDisplayValue("Two short videos");
  fireEvent.click(screen.getByRole("button",{name:"Clip 2: two.mp4"}));fireEvent.click(screen.getByText("Use player trim marks"));await act(async()=>flushProjects());
  const calls=vi.mocked(invoke).mock.calls.filter(([c])=>c==="save_workspace_video_project");const project=(calls.at(-1)![1] as {project:LoadedSequence["project"]}).project;
  expect(project.clips[0].source_in.ticks).toBe("0");expect(project.clips[1].source_in.ticks).toBe("22500");expect(project.clips[1].source_out.ticks).toBe("135000");
});
it("ignores a late project load after switching to another folder",async()=>{
  let finish:(value:LoadedSequence)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async command=>{if(command==="workspace_video_projects")return {projects:[{id:"p",title:"Saved"}],warnings:[]};if(command==="edit_workspace_video_project")return new Promise(resolve=>{finish=resolve;});});
  const ui=render(<VideoSequences roots={["/old"]} workspaceId="old"/>);await screen.findByLabelText("Saved editing projects");fireEvent.change(screen.getByLabelText("Saved editing projects"),{target:{value:"p"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"old",action:{type:"load",id:"p"}}));ui.rerender(<VideoSequences roots={["/new"]} workspaceId="new"/>);
  await act(async()=>finish(loaded()));expect(screen.queryByDisplayValue("Two short videos")).toBeNull();
});
it("protects new multi-video saves through the existing app-close save registry after unmount",async()=>{
  let finish:(revision:number)=>void=()=>{};vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const value=loaded(),session=new SequenceSession("/library",value);session.commit({...value.project,title:"Pending save"});session.release();expect(hasUnsavedProjects()).toBe(true);
  const flushed=flushProjects();finish(2);await flushed;expect(hasUnsavedProjects()).toBe(false);expect(session.revision).toBe(2);
});

it("can save an unsaved project copy after the draft save fails",async()=>{
  vi.mocked(invoke).mockImplementation(async command=>{if(command==="workspace_video_projects")return {projects:[{id:"p",title:"Saved"}],warnings:[]};if(command==="edit_workspace_video_project")return loaded();if(command==="save_workspace_video_project")throw Error("Disk full");});
  vi.mocked(save).mockResolvedValue("/backup/unsaved.json");render(<VideoSequences roots={["/library"]} workspaceId="w"/>);
  await screen.findByLabelText("Saved editing projects");fireEvent.change(screen.getByLabelText("Saved editing projects"),{target:{value:"p"}});await screen.findByDisplayValue("Two short videos");
  fireEvent.change(screen.getByLabelText("Video project name"),{target:{value:"Keep this edit"}});await screen.findByText(/Disk full/);
  fireEvent.click(screen.getByText("Save unsaved project copy…"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("write_video_sequence_copy",expect.objectContaining({destination:"/backup/unsaved.json",project:expect.objectContaining({title:"Keep this edit"})})));
});
it("loads workspace projects without source folders and uses each source's own playback root",async()=>{
  const value=loaded();value.sources[1]={...value.sources[1],root:"/second",path:"/second/two.mp4"};
  render(<VideoSequences roots={[]} workspaceId="w" initial={value}/>);
  await screen.findByDisplayValue("Two short videos");expect(screen.queryByLabelText("Video project folder")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Clip 2: two.mp4"}));expect(screen.getByText("Preview: /second/two.mp4").getAttribute("data-root")).toBe("/second");
  fireEvent.change(screen.getByLabelText("Video project name"),{target:{value:"Offline edits"}});
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({workspaceId:"w"})));
});
it("requires refreshed history after an edit and recovers a previous save as a new project copy",async()=>{
  const original=loaded(),recovered=loaded();recovered.project.id="recovered";recovered.project.title="Recovered copy";
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="workspace_video_projects")return {projects:[{id:"p",title:"Saved"}],warnings:[]};
    if(command==="edit_workspace_video_project")return (args as {action:{type:string}}).action.type==="recover"?recovered:original;
    if(command==="workspace_video_history")return [{id:42,revision:null,saved_at:null,title:"Before trim",clips:2,duration_seconds:4}];
    if(command==="save_workspace_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
  });
  render(<VideoSequences roots={["/library"]} workspaceId="w" initial={original}/>);await screen.findByDisplayValue("Two short videos");
  fireEvent.click(screen.getByText("Refresh previous saves"));await screen.findByLabelText("Previous timeline save");expect(screen.getByText(/Earlier save.*Save time unavailable/)).toBeTruthy();fireEvent.change(screen.getByLabelText("Previous timeline save"),{target:{value:"42"}});
  fireEvent.click(screen.getByText("Move later"));expect((screen.getByText("Recover timeline copy") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByText("Refresh previous saves"));await waitFor(()=>expect((screen.getByLabelText("Previous timeline save") as HTMLSelectElement).value).toBe(""));
  fireEvent.change(screen.getByLabelText("Previous timeline save"),{target:{value:"42"}});fireEvent.click(screen.getByText("Recover timeline copy"));
  await screen.findByDisplayValue("Recovered copy");expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"w",action:{type:"recover",id:"p",history_id:42,expected_revision:2}});
});
