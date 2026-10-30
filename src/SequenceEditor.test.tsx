import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {save} from "@tauri-apps/plugin-dialog";
import {VideoSequences} from "./VideoSequences";
import {defaultEdits,discardProjectSaves,flushProjects} from "./videoProject";
import {type LoadedSequence} from "./videoSequence";
import {refreshVideoExport} from "./videoExport";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(p:string)=>`asset:${p}`}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn(),save:vi.fn()}));
vi.mock("./VideoPlayer",()=>({VideoPlayer:()=>null}));
const info={duration_seconds:10,codec:"h264",width:96,height:64,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]};
function value():LoadedSequence{return {project:{schema_version:2,id:"p",title:"My edit",assets:[{id:"a",relative_path:"source.mp4",sha256:"0".repeat(64),bytes:200,info:{...info}}],clips:[{id:"c",asset_id:"a",source_in:{ticks:"0",time_base:"1/90000"},source_out:{ticks:"900000",time_base:"1/90000"},edits:defaultEdits()}],output:{width:1920,height:1080,fit:"fit"},frame_rate:30},revision:1,sources:[{asset_id:"a",root:"/library",path:"/library/source.mp4",status:"ready"}]};}
beforeEach(()=>{vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="workspace_video_projects")return {projects:[{id:"p",title:"My edit"}],warnings:[]};
  if(command==="save_workspace_video_project")return (args as {expectedRevision:number}).expectedRevision+1;
  if(command==="edit_workspace_video_project")return value();
  if(command==="video_export_status")return null;
});});
afterEach(async()=>{cleanup();discardProjectSaves();vi.mocked(invoke).mockResolvedValue(null);await refreshVideoExport();vi.resetAllMocks();});
function editor(loaded=value()){return render(<VideoSequences roots={["/library"]} workspaceId="workspace" initial={loaded}/>);}
it("uses the same timeline and controller for one source and added sources",async()=>{
  editor();await screen.findByDisplayValue("My edit");expect(screen.getByRole("button",{name:"Clip 1: source.mp4"})).toBeTruthy();
  expect((screen.getByText("Export project MP4…") as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByText("Append selected video"));expect(screen.getByRole("button",{name:"Clip 2: source.mp4"})).toBeTruthy();
  expect((screen.getByText("Export project MP4…") as HTMLButtonElement).disabled).toBe(false);expect((screen.getByLabelText("Sequence frame rate") as HTMLSelectElement).disabled).toBe(false);
  fireEvent.click(screen.getByText("Undo timeline edit"));expect((screen.getByText("Export project MP4…") as HTMLButtonElement).disabled).toBe(false);
  await act(async()=>flushProjects());
});
it("preserves invalid trim drafts and blocks rendering/export of the old interval",async()=>{
  editor();await screen.findByDisplayValue("My edit");const start=screen.getByLabelText("Clip start in source (seconds)"),end=screen.getByLabelText("Clip end in source (seconds)");
  fireEvent.change(start,{target:{value:"5"}});fireEvent.blur(start);fireEvent.change(end,{target:{value:"1"}});fireEvent.blur(end);
  expect(end.getAttribute("aria-invalid")).toBe("true");expect((end as HTMLInputElement).value).toBe("1");fireEvent.click(screen.getByText("Export project MP4…"));await screen.findByText(/Correct the highlighted field/);expect(save).not.toHaveBeenCalled();
});
it.each(["999",""])("preserves invalid volume %s and blocks stale export",async draft=>{
  editor();await screen.findByDisplayValue("My edit");const volume=screen.getByLabelText("Audio volume (%)");fireEvent.change(volume,{target:{value:draft}});fireEvent.blur(volume);
  expect((volume as HTMLInputElement).value).toBe(draft);expect(volume.getAttribute("aria-invalid")).toBe("true");fireEvent.click(screen.getByText("Export project MP4…"));await screen.findByText(/Correct the highlighted field/);expect(save).not.toHaveBeenCalled();
  fireEvent.change(volume,{target:{value:"75"}});fireEvent.blur(volume);await act(async()=>flushProjects());expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({project:expect.objectContaining({clips:[expect.objectContaining({edits:expect.objectContaining({gain_percent:75})})]})}));
});
it("accepts rational source endpoints between milliseconds without blocking export",async()=>{
  const loaded=value();loaded.project.assets[0].info.duration_seconds=Number((900901/90000).toFixed(6));loaded.project.clips[0].source_out.ticks="900901";
  vi.mocked(save).mockResolvedValue(null);editor(loaded);await screen.findByDisplayValue("My edit");const end=screen.getByLabelText("Clip end in source (seconds)") as HTMLInputElement;fireEvent.blur(end);expect(end.validity.valid).toBe(true);
  fireEvent.click(screen.getByText("Export project MP4…"));await waitFor(()=>expect(save).toHaveBeenCalledOnce());expect(loaded.project.clips[0].source_out.ticks).toBe("900901");
});
it("retains transforms, adjustments, crop and linked audio through shared undo/autosave",async()=>{
  editor();await screen.findByDisplayValue("My edit");fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});fireEvent.click(screen.getByLabelText("Flip horizontally"));
  const brightness=screen.getByLabelText("Video brightness"),contrast=screen.getByLabelText("Video contrast");fireEvent.change(brightness,{target:{value:"20"}});fireEvent.blur(brightness);fireEvent.change(contrast,{target:{value:"-30"}});fireEvent.blur(contrast);
  fireEvent.click(screen.getByText("Reset brightness and contrast"));expect((brightness as HTMLInputElement).value).toBe("0");fireEvent.click(screen.getByText("Undo timeline edit"));expect((brightness as HTMLInputElement).value).toBe("20");
  fireEvent.click(screen.getByText("Square crop"));await act(async()=>flushProjects());
  expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({project:expect.objectContaining({clips:[expect.objectContaining({edits:expect.objectContaining({brightness:20,contrast:-30,quarter_turns:1,flip_horizontal:true})})]})}));
});
it("ignores an obsolete preview result after editing and cancels its native job",async()=>{
  const fallback=vi.mocked(invoke).getMockImplementation()!;let finish:(value:unknown)=>void=()=>{};
  vi.mocked(invoke).mockImplementation(async(command,args)=>{if(command==="start_workspace_video_preview")return "preview";if(command==="video_preview_status")return new Promise(resolve=>{finish=resolve});return fallback(command,args);});
  editor();await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Preview project edits"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("video_preview_status",{id:"preview"}));
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});await act(async()=>finish({state:"complete",result:{source:"obsolete.mp4",warnings:[]},error:null}));
  expect(screen.queryByLabelText("Edited project preview")).toBeNull();expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"preview"});
});
it("cannot start an obsolete render when a newer edit arrives during autosave",async()=>{
  const fallback=vi.mocked(invoke).getMockImplementation()!;let finish:(revision:number)=>void=()=>{},saves=0;
  vi.mocked(invoke).mockImplementation(async(command,args)=>{if(command==="save_workspace_video_project"&&++saves===1)return new Promise(resolve=>{finish=resolve});if(command==="start_workspace_video_preview")return "new";if(command==="video_preview_status")return {state:"complete",result:{source:"new.mp4",warnings:[]}};return fallback(command,args);});
  editor();await screen.findByDisplayValue("My edit");fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});fireEvent.click(screen.getByText("Preview project edits"));fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"2"}});await act(async()=>finish(2));await act(async()=>flushProjects());
  expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="start_workspace_video_preview")).toBe(false);fireEvent.click(screen.getByText("Preview project edits"));await screen.findByLabelText("Edited project preview");
  expect(invoke).toHaveBeenCalledWith("start_workspace_video_preview",{workspaceId:"workspace",id:"p",expectedRevision:3});
});
it("exports a saved revision and keeps its native job alive after closing the editor",async()=>{
  const fallback=vi.mocked(invoke).getMockImplementation()!;vi.mocked(save).mockResolvedValue("/output/new.mp4");
  vi.mocked(invoke).mockImplementation(async(command,args)=>command==="start_workspace_video_export"?"export-job":fallback(command,args));
  const ui=editor();await screen.findByDisplayValue("My edit");fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});fireEvent.click(screen.getByText("Export project MP4…"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("start_workspace_video_export",{workspaceId:"workspace",id:"p",expectedRevision:2,options:{destination:"/output/new.mp4",size:"project",acknowledge_first_audio:false}}));
  ui.unmount();expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="cancel_video_export")).toBe(false);
});
it.each(["editing","closing"])("does not export after %s while choosing a destination",async mode=>{
  let finish:(path:string)=>void=()=>{};vi.mocked(save).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
  const ui=editor();await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Export project MP4…"));await waitFor(()=>expect(save).toHaveBeenCalledOnce());fireEvent.click(screen.getByText("Export project MP4…"));expect(save).toHaveBeenCalledOnce();
  if(mode==="closing")ui.unmount();else fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});
  await act(async()=>finish("/output/late.mp4"));if(mode==="editing")await screen.findByText(/project changed while choosing/);expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="start_workspace_video_export")).toBe(false);
  ui.unmount();
});
it("requires explicit first-track audio choice unless muted",async()=>{
  const loaded=value();loaded.project.assets[0].info.audio=[1,2].map(stream=>({stream,codec:"aac",sample_rate:"48000",channels:2,channel_layout:"stereo"}));editor(loaded);await screen.findByDisplayValue("My edit");
  const button=screen.getByText("Export project MP4…") as HTMLButtonElement;expect(button.disabled).toBe(true);fireEvent.click(screen.getByLabelText("Confirm first audio track only"));expect(button.disabled).toBe(false);fireEvent.click(screen.getByLabelText("Confirm first audio track only"));fireEvent.click(screen.getByLabelText("Mute video audio"));expect(button.disabled).toBe(false);
  await act(async()=>flushProjects());
});
it.each(["hdr","missing"])("disables %s source rendering without blocking project edits",async reason=>{
  const loaded=value();if(reason==="hdr")loaded.project.assets[0].info.hdr=true;else loaded.sources[0]={asset_id:"a",path:null,root:null,status:"Source missing"};
  editor(loaded);await screen.findByDisplayValue("My edit");expect((screen.getByText("Preview project edits") as HTMLButtonElement).disabled).toBe(true);expect((screen.getByText("Export project MP4…") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Video rotation"),{target:{value:"1"}});await act(async()=>flushProjects());expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.anything());
});
it("keeps 4K, source canvas and fit/fill settings without modifying source metadata",async()=>{
  const loaded=value();loaded.project.assets[0].info={...info,width:3840,height:1080};editor(loaded);await screen.findByDisplayValue("My edit");fireEvent.change(screen.getByLabelText("Video canvas"),{target:{value:"3840x2160"}});fireEvent.change(screen.getByLabelText("Video canvas"),{target:{value:"3840x1080"}});fireEvent.change(screen.getByLabelText("Video canvas placement"),{target:{value:"fill"}});await act(async()=>flushProjects());
  expect(invoke).toHaveBeenCalledWith("save_workspace_video_project",expect.objectContaining({project:expect.objectContaining({output:{width:3840,height:1080,fit:"fill"}})}));expect(loaded.project.output.width).toBe(1920);
});
it("previews a joined sequence on one stage and connects playback to the project playhead",async()=>{
  const fallback=vi.mocked(invoke).getMockImplementation()!,loaded=value();loaded.project.clips.push({...structuredClone(loaded.project.clips[0]),id:"second"});
  vi.mocked(invoke).mockImplementation(async(command,args)=>command==="start_workspace_video_preview"?"joined-preview":command==="video_preview_status"?{state:"complete",result:{source:"joined.mp4",warnings:[]}}:fallback(command,args));
  const ui=editor(loaded);await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Preview project edits"));const video=await screen.findByLabelText("Edited project preview") as HTMLVideoElement;
  expect(ui.container.querySelectorAll("video")).toHaveLength(1);fireEvent.change(screen.getByLabelText("Project playhead"),{target:{value:"0.5"}});expect(video.currentTime).toBe(0.5);
  video.currentTime=0.75;fireEvent.timeUpdate(video);expect((screen.getByLabelText("Project playhead") as HTMLInputElement).value).toBe("0.75");
  fireEvent.click(screen.getByText("Show original source"));expect(screen.queryByLabelText("Edited project preview")).toBeNull();await waitFor(()=>expect(video.getAttribute("src")).toBeNull());expect(invoke).toHaveBeenCalledWith("cancel_video_preview",{id:"joined-preview"});
});
it("keeps export presets outside the saved project and exports the whole joined revision",async()=>{
  const fallback=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation(async(command,args)=>command==="start_workspace_video_export"?"joined-export":fallback(command,args));vi.mocked(save).mockResolvedValue("/output/joined.mp4");
  editor();await screen.findByDisplayValue("My edit");fireEvent.click(screen.getByText("Append selected video"));await act(async()=>flushProjects());
  fireEvent.change(screen.getByLabelText("Sequence frame rate"),{target:{value:"60"}});await act(async()=>flushProjects());fireEvent.change(screen.getByLabelText("Video canvas"),{target:{value:"3840x2160"}});await act(async()=>flushProjects());
  const saves=vi.mocked(invoke).mock.calls.filter(([c])=>c==="save_workspace_video_project").length;fireEvent.change(screen.getByLabelText("Video export size"),{target:{value:"hd1080"}});expect(screen.getByText(/Export resolution: 1920 × 1080/)).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="save_workspace_video_project")).toHaveLength(saves);fireEvent.click(screen.getByText("Export project MP4…"));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("start_workspace_video_export",{workspaceId:"workspace",id:"p",expectedRevision:4,options:{destination:"/output/joined.mp4",size:"hd1080",acknowledge_first_audio:false}}));
});
it("does not require an unused missing bin source to render the timeline",async()=>{
  const loaded=value(),unused={...structuredClone(loaded.project.assets[0]),id:"unused",relative_path:"offline.mp4",sha256:"1".repeat(64)};loaded.project.assets.push(unused);loaded.sources.push({asset_id:unused.id,path:null,root:null,status:"Offline"});
  editor(loaded);await screen.findByDisplayValue("My edit");expect((screen.getByText("Export project MP4…") as HTMLButtonElement).disabled).toBe(false);
});
