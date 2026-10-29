import {StrictMode} from "react";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
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
function scan(root:string,count=1){return {scan:{photos:Array.from({length:count},(_,i)=>({path:`${root}\\photo-${i}.jpg`,name:`${root===A?"alpha":root===B?"beta":"gamma"}-${i}.jpg`,extension:"jpg",bytes:100,modified_unix:1700000000})),scanned_files:count,truncated:false,incomplete:false,errors:[]},changes:{added:count,updated:0,unchanged:0,removed:0,incomplete:false}};}
const models=["semantic","semantic_text","semantic_tokenizer","faces","face_embedder","ocr","ocr_recognizer"].map(kind=>({kind,installed:true,compatible:true,bytes:10,sha256:"0".repeat(64),expected_sha256:"0".repeat(64)}));
async function fallback(command:string,args:unknown){
  if(command==="scan_indexed_with_changes")return scan((args as {root:string}).root);
  if(command==="preview_image")return "data:image/png;base64,preview";
  if(command==="ai_model_status")return models;
  if(command==="ai_runtime_status")return {directml_compiled:true,directml_available:false,cpu_fallback:true,automatic_policy:"CPU fallback"};
  return [];
}
beforeEach(()=>{
  localStorage.clear();localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace()));localStorage.setItem("openphoto.activeWorkspace","library");
  vi.stubGlobal("matchMedia",()=>({matches:false,addEventListener:()=>{},removeEventListener:()=>{}}));
  vi.mocked(invoke).mockImplementation(fallback);
});
afterEach(async()=>{cleanup();await act(async()=>{});vi.resetAllMocks();vi.restoreAllMocks();vi.unstubAllGlobals();});
function openFolders(){fireEvent.click(screen.getByRole("button",{name:"Folders"}));}
function createWorkspaceFromFolders(name:string){openFolders();fireEvent.change(screen.getByRole("combobox",{name:"Active workspace"}),{target:{value:"__new__"}});fireEvent.change(screen.getByLabelText("Workspace name"),{target:{value:name}});fireEvent.click(screen.getByRole("button",{name:"Create"}));}
it("resets both the viewport and virtual rows when sorting a scrolled library",async()=>{
  vi.mocked(invoke).mockImplementation((command,args)=>command==="scan_indexed_with_changes"?Promise.resolve(scan((args as {root:string}).root,60)):fallback(command,args));
  const {container}=render(<App/>);await screen.findByText("alpha-0.jpg");const grid=container.querySelector<HTMLElement>(".photo-grid")!;
  grid.scrollTop=2000;fireEvent.scroll(grid);expect(screen.queryByText("alpha-0.jpg")).toBeNull();
  fireEvent.change(screen.getByRole("combobox",{name:"Sort media"}),{target:{value:"name"}});
  expect(grid.scrollTop).toBe(0);expect(screen.getByText("alpha-0.jpg")).toBeTruthy();
});
it("restores saved folders and their contents once on startup under StrictMode",async()=>{
  render(<StrictMode><App/></StrictMode>);
  await screen.findByText("alpha-0.jpg");expect(screen.getByText("beta-0.jpg")).toBeTruthy();
  const scans=vi.mocked(invoke).mock.calls.filter(([command])=>command==="scan_indexed_with_changes");
  expect(scans.map(([,args])=>(args as {root:string}).root).sort()).toEqual([A,B].sort());
  openFolders();expect(screen.getByRole("button",{name:`Remove ${A} from library`})).toBeTruthy();
  expect(await screen.findByText("v0.56.2 preview")).toBeTruthy();
});
it("clears scan status when creating an empty workspace and rejects the previous workspace's pending scan",async()=>{
  let finish:(value:ReturnType<typeof scan>)=>void=()=>{};
  vi.mocked(invoke).mockImplementation((command,args)=>command==="scan_indexed_with_changes"&&(args as {root:string}).root===B?new Promise(resolve=>{finish=resolve}):fallback(command,args));
  render(<App/>);await waitFor(()=>expect(invoke).toHaveBeenCalledWith("scan_indexed_with_changes",{root:B}));
  createWorkspaceFromFolders("Video acceptance");
  await act(async()=>finish(scan(B)));
  openFolders();expect(screen.getByRole("combobox",{name:"Active workspace"}).textContent).toContain("Video acceptance");
  expect(screen.queryByText("alpha-0.jpg")).toBeNull();expect(screen.queryByText("beta-0.jpg")).toBeNull();
  expect(screen.queryByText(/Last indexed:/)).toBeNull();expect(screen.queryByText(/Changes:/)).toBeNull();
  expect(screen.getByText("No folders added. Choose Add folder to start.")).toBeTruthy();
});
it("does not carry a completed scan's counts or timestamp into a new empty workspace",async()=>{
  render(<App/>);
  await screen.findByText("alpha-0.jpg");openFolders();expect(screen.getByText(/Last indexed:/)).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox",{name:"Active workspace"}),{target:{value:"__new__"}});
  fireEvent.change(screen.getByLabelText("Workspace name"),{target:{value:"Empty workspace"}});fireEvent.click(screen.getByRole("button",{name:"Create"}));
  expect(screen.queryByText(/Last indexed:/)).toBeNull();expect(screen.queryByText(/Changes:/)).toBeNull();
});
it("removes a folder during scanning, rejects the stale result, and does not resurrect removed roots",async()=>{
  let finish:(value:ReturnType<typeof scan>)=>void=()=>{};
  vi.mocked(invoke).mockImplementation((command,args)=>command==="scan_indexed_with_changes"&&(args as {root:string}).root===B?new Promise(resolve=>{finish=resolve}):fallback(command,args));
  const view=render(<App/>);
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("scan_indexed_with_changes",{root:B}));
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
    if(command==="scan_indexed_with_changes")return Promise.resolve(scan(A,129));
    if(command==="ai_index_semantic_batch")return new Promise(resolve=>{finish=resolve});
    return fallback(command,args);
  });
  render(<App/>);await screen.findByText("alpha-0.jpg");
  const start=screen.getByRole("button",{name:"Index library with AI"});await waitFor(()=>expect((start as HTMLButtonElement).disabled).toBe(false));fireEvent.click(start);
  await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="ai_index_semantic_batch")).toHaveLength(1));
  fireEvent.click(screen.getByRole("button",{name:"Settings"}));fireEvent.click(await screen.findByRole("switch",{name:"Local AI"}));
  await act(async()=>finish({requested:64,indexed:64,skipped:0,failed:0,errors:[]}));
  expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==="ai_index_semantic_batch")).toHaveLength(1);
  expect(localStorage.getItem("openphoto.ai.optin")).toBe("false");expect((screen.getByRole("switch",{name:"Local AI"}) as HTMLInputElement).checked).toBe(false);
});
