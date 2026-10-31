import {StrictMode} from "react";
import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
vi.hoisted(()=>{(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__={};});
import {invoke} from "@tauri-apps/api/core";
import {open} from "@tauri-apps/plugin-dialog";
import App from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(path:string)=>path}));
vi.mock("@tauri-apps/api/app",()=>({getVersion:async()=>"0.56.2"}));
vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn(),save:vi.fn()}));
vi.mock("@tauri-apps/api/event",()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock("./PhotoMap",()=>({default:()=>null}));
vi.mock("./VideoExportPanel",()=>({VideoExportPanel:()=>null}));
vi.mock("./ProjectSaveGuard",()=>({ProjectSaveGuard:()=>null}));
const A="C:\\Media A",B="C:\\Media B",C="C:\\Media C";
const workspace=(roots=[A,B])=>[{id:"library",name:"My library",root:roots[0]||"",roots}];
function scan(root:string,count=1){return {photos:Array.from({length:count},(_,i)=>({path:`${root}\\photo-${i}.jpg`,name:`${root===A?"alpha":root===B?"beta":"gamma"}-${i}.jpg`,extension:"jpg",bytes:100,modified_unix:1700000000})),scanned_files:count,truncated:false,incomplete:false,errors:[]};}
const models=["semantic","semantic_text","semantic_tokenizer","faces","face_embedder","ocr","ocr_recognizer"].map(kind=>({kind,installed:true,compatible:true,bytes:10,sha256:"0".repeat(64),expected_sha256:"0".repeat(64)}));
async function fallback(command:string,args:unknown){
  if(command==="saved_library")return scan((args as {root:string}).root);
  if(command==="scan_indexed_with_changes")return {scan:scan((args as {root:string}).root),changes:{added:1,updated:0,unchanged:0,removed:0,incomplete:false}};
  if(command==="preview_image")return "data:image/png;base64,preview";
  if(command==="ai_model_status")return models;
  if(command==="ai_runtime_status")return {directml_compiled:true,directml_available:false,cpu_fallback:true,automatic_policy:"CPU fallback"};
  return [];
}
beforeEach(()=>{vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  localStorage.clear();localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace()));localStorage.setItem("openphoto.activeWorkspace","library");
  vi.stubGlobal("matchMedia",()=>({matches:false,addEventListener:()=>{},removeEventListener:()=>{}}));
  vi.mocked(invoke).mockImplementation(fallback);
});
afterEach(async()=>{cleanup();await act(async()=>{});vi.resetAllMocks();vi.restoreAllMocks();vi.unstubAllGlobals();});
function openFolders(){fireEvent.click(screen.getByRole("button",{name:"Folders"}));}
function createWorkspaceFromFolders(name:string){openFolders();fireEvent.change(screen.getByRole("combobox",{name:"Active workspace"}),{target:{value:"__new__"}});fireEvent.change(screen.getByLabelText("Workspace name"),{target:{value:name}});fireEvent.click(screen.getByRole("button",{name:"Create"}));}
it("persists a stock accent choice from Settings",async()=>{
  render(<App/>);await screen.findByText("alpha-0.jpg");fireEvent.click(screen.getByRole("button",{name:"Settings"}));
  fireEvent.click(screen.getByRole("button",{name:"Blue accent"}));
  await waitFor(()=>expect(localStorage.getItem("openphoto.accent")).toBe("blue"));
  expect(document.documentElement.dataset.accent).toBe("blue");
  expect(screen.getByRole("button",{name:"Blue accent"}).getAttribute("aria-pressed")).toBe("true");
});
it("loads video motion only after hovering a Library video tile",async()=>{
  localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="saved_library"){const value=scan(A);value.photos=value.photos.map(p=>({...p,path:p.path.replace(".jpg",".mp4"),name:p.name.replace(".jpg",".mp4"),extension:"mp4"}));return value;}
    if(command==="video_poster")return "data:image/png;base64,poster";
    if(command==="video_source")return "/hover.mp4";
    return fallback(command,args);
  });
  const {container}=render(<App/>);await screen.findByText("alpha-0.mp4");
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="video_source")).toBe(false);
  fireEvent.pointerEnter(container.querySelector(".library-hover-preview")!);
  expect(await screen.findByLabelText("Preview alpha-0.mp4")).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("video_source",{root:A,path:`${A}\\photo-0.mp4`});
});
it("keeps section labels consistent while navigating the main pages",async()=>{
  const {container}=render(<App/>);await screen.findByText("alpha-0.jpg");
  for(const [page,label] of [["Library","YOUR COLLECTION"],["Folders","LIBRARY ORGANIZATION"],["Map","PHOTO LOCATIONS"],["Collections","VIRTUAL ORGANIZATION"],["By date","BY CAPTURE DATE"],["Memories","ON THIS DAY"],["People","LOCAL PEOPLE INDEX"],["Tags","PORTABLE METADATA"],["Settings","PREFERENCES"]]){
    fireEvent.click(screen.getByRole("button",{name:page}));expect(container.querySelector(".page-heading .eyebrow")?.textContent).toBe(label);
  }
});
it("keeps workspace selection at the top of the sidebar and opens editing from the Library",async()=>{
  const {container}=render(<App/>);await screen.findByText("alpha-0.jpg");
  const selector=screen.getByRole("combobox",{name:"Active workspace"});expect(selector.closest(".sidebar")).toBeTruthy();
  expect(screen.queryByRole("button",{name:"Video projects"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Edits"}));
  expect(screen.getByRole("heading",{name:"Video editor"})).toBeTruthy();
  expect(container.querySelector('.sidebar select')).toBe(selector);
  fireEvent.click(screen.getByRole("button",{name:"Back to Library"}));expect(screen.getByRole("heading",{name:"Media library"})).toBeTruthy();
});
it("creates one timeline project from multiple selected Library videos",async()=>{
  localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="saved_library"){const value=scan(A,2);value.photos=value.photos.map(p=>({...p,path:p.path.replace(".jpg",".mp4"),name:p.name.replace(".jpg",".mp4"),extension:"mp4"}));return value;}
    if(command==="edit_workspace_video_project")return new Promise(()=>{});
    return fallback(command,args);
  });
  render(<App/>);await screen.findByText("alpha-0.mp4");fireEvent.click(screen.getByRole("button",{name:"Select matching"}));
  fireEvent.click(screen.getByRole("button",{name:"Create video from selection…"}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"library",action:{type:"create",paths:[`${A}\\photo-0.mp4`,`${A}\\photo-1.mp4`]}}));
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="edit_workspace_video_project")).toHaveLength(1);
});
it("creates one continuous project from a mixed Library selection",async()=>{
  localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="saved_library"){const value=scan(A,2);value.photos[1]={...value.photos[1],path:`${A}\\video.mp4`,name:"video.mp4",extension:"mp4"};return value;}
    if(command==="edit_workspace_video_project")return new Promise(()=>{});return fallback(command,args);
  });
  render(<App/>);await screen.findByText("alpha-0.jpg");expect(screen.getByText("Image",{exact:true})).toBeTruthy();expect(screen.getByText("Video",{exact:true})).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"Select matching"}));fireEvent.click(screen.getByRole("button",{name:"Create video from selection…"}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"library",action:{type:"create",paths:[`${A}\\photo-0.jpg`,`${A}\\video.mp4`]}}));
});
it("opens a selected video through the shared workspace project route",async()=>{
  localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="inspect_video")return {duration_seconds:1,codec:"h264",width:96,height:64,frame_rate:"30/1",nominal_frame_rate:"30/1",time_base:"1/90000",sample_aspect_ratio:"1:1",rotation_degrees:0,hdr:false,color_transfer:null,audio:[],warnings:[]};
    if(command==="video_source")return "/fixture.mp4";
    if(command==="start_video_preview")return "fixture-preview";
    if(command==="video_preview_status")return {state:"failed",result:null,error:"This route fixture has no playback media"};
    if(command==="saved_library"){const value=scan(A);value.photos=value.photos.map(p=>({...p,path:p.path.replace(".jpg",".mp4"),name:p.name.replace(".jpg",".mp4"),extension:"mp4"}));return value;}
    if(command==="edit_workspace_video_project")return new Promise(()=>{});return fallback(command,args);
  });
  render(<App/>);fireEvent.click(await screen.findByText("alpha-0.mp4"));fireEvent.click(screen.getByRole("button",{name:"Edit"}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("edit_workspace_video_project",{workspaceId:"library",action:{type:"open_source",path:`${A}\\photo-0.mp4`}}));
  expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="create_video_project")).toBe(false);
});
it("uses the Library search for tags and favorites, shows match reasons and keeps filters out of the index commands",async()=>{
  localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));localStorage.setItem("openphoto.ratings",JSON.stringify({[`${A}\\photo-1.jpg`]:5}));
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="saved_library")return scan(A,2);
    if(command==="read_managed_tags")return [{path:`${A}\\photo-1.jpg`,tags:["food/burger"]}];return fallback(command,args);
  });
  render(<App/>);await screen.findByText("alpha-0.jpg");fireEvent.change(screen.getByLabelText("Sort media"),{target:{value:"largest"}});const search=screen.getByRole("textbox",{name:"Search library"});fireEvent.change(search,{target:{value:"burger"}});
  await waitFor(()=>expect(screen.queryByText("alpha-0.jpg")).toBeNull());expect(screen.queryByText("alpha-0.jpg")).toBeNull();expect(screen.getByText("alpha-1.jpg")).toBeTruthy();expect((screen.getByLabelText("Sort media") as HTMLSelectElement).value).toBe("relevance");
  fireEvent.change(search,{target:{value:"is:favorite"}});await waitFor(()=>expect(screen.getByText("5 stars",{exact:false})).toBeTruthy());expect(screen.queryByText("alpha-0.jpg")).toBeNull();
  fireEvent.change(search,{target:{value:"rating:6"}});expect(screen.getByText(/Use type:photo/)).toBeTruthy();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="search_workspace_evidence")).toBe(false);
  expect(screen.queryByLabelText("Semantic photo search")).toBeNull();expect(screen.queryByLabelText("OCR photo search")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Clear search"}));expect((screen.getByLabelText("Sort media") as HTMLSelectElement).value).toBe("largest");expect(screen.getByText("alpha-0.jpg")).toBeTruthy();
});
it("resets both the viewport and virtual rows when sorting a scrolled library",async()=>{
  vi.mocked(invoke).mockImplementation((command,args)=>command==="saved_library"?Promise.resolve(scan((args as {root:string}).root,60)):fallback(command,args));
  const {container}=render(<App/>);await screen.findByText("alpha-0.jpg");const grid=container.querySelector<HTMLElement>(".photo-grid")!;
  grid.scrollTop=2000;fireEvent.scroll(grid);expect(screen.queryByText("alpha-0.jpg")).toBeNull();
  fireEvent.change(screen.getByRole("combobox",{name:"Sort media"}),{target:{value:"name"}});
  expect(grid.scrollTop).toBe(0);expect(screen.getByText("alpha-0.jpg")).toBeTruthy();
});
it("restores saved folders and their contents once on startup under StrictMode",async()=>{
  render(<StrictMode><App/></StrictMode>);
  await screen.findByText("alpha-0.jpg");expect(screen.getByText("beta-0.jpg")).toBeTruthy();
  const scans=vi.mocked(invoke).mock.calls.filter(([command])=>command==="saved_library");
  expect(scans.map(([,args])=>(args as {root:string}).root).sort()).toEqual([A,B].sort());
  openFolders();expect(screen.getByRole("button",{name:`Remove ${A} from library`})).toBeTruthy();
  expect(await screen.findByText("v0.56.2 preview")).toBeTruthy();
});
it("clears scan status when creating an empty workspace and rejects the previous workspace's pending scan",async()=>{
  let finish:(value:ReturnType<typeof scan>)=>void=()=>{};
  vi.mocked(invoke).mockImplementation((command,args)=>command==="saved_library"&&(args as {root:string}).root===B?new Promise(resolve=>{finish=resolve}):fallback(command,args));
  render(<App/>);await waitFor(()=>expect(invoke).toHaveBeenCalledWith("saved_library",{root:B}));
  createWorkspaceFromFolders("Video acceptance");
  await act(async()=>finish(scan(B)));
  openFolders();expect(screen.getByRole("combobox",{name:"Active workspace"}).textContent).toContain("Video acceptance");
  expect(screen.queryByText("alpha-0.jpg")).toBeNull();expect(screen.queryByText("beta-0.jpg")).toBeNull();
  expect(screen.queryByText(/Last synced:/)).toBeNull();expect(screen.queryByText(/Changes:/)).toBeNull();
  expect(screen.getByText("No folders added. Choose Add folder to start.")).toBeTruthy();
});
it("does not carry a completed scan's counts or timestamp into a new empty workspace",async()=>{
  render(<App/>);
  await screen.findByText("alpha-0.jpg");fireEvent.click(screen.getByRole("button",{name:"Sync"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("scan_indexed_with_changes",{root:A}));openFolders();await screen.findByText(/Last synced:/);
  fireEvent.change(screen.getByRole("combobox",{name:"Active workspace"}),{target:{value:"__new__"}});
  fireEvent.change(screen.getByLabelText("Workspace name"),{target:{value:"Empty workspace"}});fireEvent.click(screen.getByRole("button",{name:"Create"}));
  expect(screen.queryByText(/Last synced:/)).toBeNull();expect(screen.queryByText(/Changes:/)).toBeNull();
});
it("removes a folder during scanning, rejects the stale result, and does not resurrect removed roots",async()=>{
  let finish:(value:ReturnType<typeof scan>)=>void=()=>{};
  vi.mocked(invoke).mockImplementation((command,args)=>command==="saved_library"&&(args as {root:string}).root===B?new Promise(resolve=>{finish=resolve}):fallback(command,args));
  const view=render(<App/>);
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("saved_library",{root:B}));
  openFolders();fireEvent.click(screen.getByRole("button",{name:`Remove ${B} from library`}));
  await act(async()=>finish(scan(B)));fireEvent.click(screen.getByRole("button",{name:"Library"}));await screen.findByText("alpha-0.jpg");expect(screen.queryByText("beta-0.jpg")).toBeNull();
  openFolders();fireEvent.click(screen.getByRole("button",{name:`Remove ${A} from library`}));
  expect(await screen.findByText(/No folders added/)).toBeTruthy();
  await waitFor(()=>expect(JSON.parse(localStorage.getItem("openphoto.workspaces")!)[0].roots).toEqual([]));
  view.unmount();render(<App/>);openFolders();expect(await screen.findByText(/No folders added/)).toBeTruthy();
  vi.mocked(open).mockResolvedValue(C);fireEvent.click(screen.getByRole("button",{name:"+ Add folder"}));
  fireEvent.click(screen.getByRole("button",{name:"Library"}));await screen.findByText("gamma-0.jpg");expect(screen.queryByText("alpha-0.jpg")).toBeNull();
  expect(JSON.parse(localStorage.getItem("openphoto.workspaces")!)[0].roots).toEqual([C]);
});
it("stops scheduling AI batches when the switch is turned off and retains that preference",async()=>{
  localStorage.setItem("openphoto.ai.optin","true");localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
  let finish:(value:unknown)=>void=()=>{};
  vi.mocked(invoke).mockImplementation((command,args)=>{
    if(command==="saved_library")return Promise.resolve(scan(A,129));
    if(command==="ai_index_semantic_batch")return new Promise(resolve=>{finish=resolve});
    return fallback(command,args);
  });
  render(<App/>);await screen.findByText("alpha-0.jpg");
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));const start=screen.getByRole("button",{name:"Index library with AI"});await waitFor(()=>expect((start as HTMLButtonElement).disabled).toBe(false));fireEvent.click(start);
  await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="ai_index_semantic_batch")).toHaveLength(1));
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));fireEvent.click(await screen.findByRole("switch",{name:"Local AI"}));
  await act(async()=>finish({requested:64,indexed:64,skipped:0,failed:0,errors:[]}));
  expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="ai_index_semantic_batch")).toHaveLength(1);
  expect(localStorage.getItem("openphoto.ai.optin")).toBe("false");expect((screen.getByRole("switch",{name:"Local AI"}) as HTMLInputElement).checked).toBe(false);
});

it("ignores old auto-sync settings, restores saved media, and scans only on manual Sync",async()=>{
 localStorage.setItem("openphoto.autosync","true");const intervals=vi.spyOn(window,"setInterval");
 render(<App/>);await screen.findByText("alpha-0.jpg");expect(invoke).toHaveBeenCalledWith("set_watched_folders",{roots:[]});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="scan_indexed_with_changes"||c==="update_indexed_path")).toBe(false);
 expect(intervals.mock.calls.some(([,delay])=>delay===120000)).toBe(false);expect(localStorage.getItem("openphoto.autosync")).toBe("false");
 expect(screen.queryByText("Search tips")).toBeNull();expect(screen.queryByText("Search and library tools")).toBeNull();expect(screen.queryByRole("button",{name:"Open video editor…"})).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"Sync"}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="scan_indexed_with_changes")).toHaveLength(2));
 openFolders();expect(screen.queryByRole("checkbox",{name:/Auto-sync/})).toBeNull();expect(screen.getByRole("button",{name:"Find Exact Duplicates"})).toBeTruthy();
});
it("retains a failed folder's known media during a partial manual sync",async()=>{
 render(<App/>);await screen.findByText("alpha-0.jpg");await screen.findByText("beta-0.jpg");
 vi.mocked(invoke).mockImplementation(async(command,args)=>{if(command==="scan_indexed_with_changes"&&(args as {root:string}).root===B)throw Error("Drive offline");return fallback(command,args);});
 fireEvent.click(screen.getByRole("button",{name:"Sync"}));await screen.findByText(/Drive offline/);expect(screen.getByText("alpha-0.jpg")).toBeTruthy();expect(screen.getByText("beta-0.jpg")).toBeTruthy();
});
it("remembers independent folder and collection views and retains their actions",async()=>{
 localStorage.setItem("openphoto.collections",JSON.stringify([{id:"c",workspaceId:"library",name:"Holiday",paths:[`${A}\\photo-0.jpg`]}]));
 localStorage.setItem("openphoto.smartRules",JSON.stringify([{id:"s",workspaceId:"library",name:"Rated",type:"all",minRating:1,minMB:0}]));
 const app=render(<App/>);await screen.findByText("alpha-0.jpg");openFolders();const folders=screen.getByRole("group",{name:"Folder view"});fireEvent.click(within(folders).getByRole("button",{name:"Tiles"}));
 expect(localStorage.getItem("openphoto.browse.folders.library")).toBe("tiles");expect(app.container.querySelector(".browse-items.tiles")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"Collections"}));const collections=screen.getByRole("group",{name:"Collection view"});expect(within(collections).getByRole("button",{name:"List"}).getAttribute("aria-pressed")).toBe("true");fireEvent.click(within(collections).getByRole("button",{name:"Tiles"}));
 expect(app.container.querySelectorAll(".browse-items.tiles")).toHaveLength(3);expect(screen.getAllByRole("button",{name:"Remove"})).toHaveLength(2);
 fireEvent.click(screen.getByRole("button",{name:/Holiday/}));expect(await screen.findByText("alpha-0.jpg")).toBeTruthy();expect(screen.queryByText("beta-0.jpg")).toBeNull();
 app.unmount();render(<App/>);await screen.findByText("alpha-0.jpg");openFolders();expect(within(screen.getByRole("group",{name:"Folder view"})).getByRole("button",{name:"Tiles"}).getAttribute("aria-pressed")).toBe("true");
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="scan_indexed_with_changes")).toBe(false);
});
