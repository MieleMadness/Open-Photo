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

it("changed source_version refreshes a same-size same-second thumbnail",async()=>{
 const photo={...scan(A).photos[0],path:A+"\\identity.jpg",name:"identity.jpg",source_version:"source-v1:old"};
 localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="saved_library")return {photos:[photo],scanned_files:1,truncated:false};
  if(command==="scan_indexed_with_changes")return {scan:{photos:[{...photo,source_version:"source-v1:new"}],scanned_files:1,truncated:false},changes:{added:0,updated:1,unchanged:0,removed:0,incomplete:false}};
  return fallback(command,args);
 });
 render(<App/>);await screen.findByText("identity.jpg");
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c,a])=>c==="preview_image"&&(a as {path:string}).path===photo.path)).toHaveLength(1));
 openFolders();fireEvent.click(screen.getByRole("button",{name:"Sync now"}));
 await screen.findByText(/Changes:.*1 updated/);
 fireEvent.click(screen.getByRole("button",{name:"Library"}));
 await screen.findByText("identity.jpg");await act(async()=>{});
 expect(vi.mocked(invoke).mock.calls.filter(([c,a])=>c==="preview_image"&&(a as {path:string}).path===photo.path)).toHaveLength(2);
});
it("duplicate results from an old workspace cannot repopulate the new workspace",async()=>{
 let finish:(value:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementation(async(command,args)=>command==="find_workspace_duplicates"?new Promise(resolve=>{finish=resolve}):fallback(command,args));
 localStorage.setItem("openphoto.workspaces",JSON.stringify([...workspace([A]),{id:"second",name:"Second workspace",root:B,roots:[B]}]));
 render(<App/>);await screen.findByText("alpha-0.jpg");openFolders();
 fireEvent.click(screen.getByRole("button",{name:"Find Exact Duplicates"}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("find_workspace_duplicates",expect.anything()));
 fireEvent.change(screen.getByRole("combobox",{name:"Active workspace"}),{target:{value:"second"}});
 await screen.findByText("beta-0.jpg");
 await act(async()=>finish([{paths:[A+"\\photo-0.jpg",B+"\\photo-0.jpg"],bytes:100}]));
 openFolders();
 expect(screen.queryByText(A+"\\photo-0.jpg")).toBeNull();
});
it("root folder selection excludes nested folders",async()=>{
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="saved_library")return {photos:[...scan(A).photos,{...scan(A).photos[0],path:A+"\\Trips\\nested.jpg",name:"nested.jpg"}],scanned_files:2,truncated:false};
  return fallback(command,args);
 });
 localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
 render(<App/>);await screen.findByText("nested.jpg");openFolders();
 fireEvent.click(screen.getByRole("button",{name:"(root)"}));
 expect(screen.queryByText("nested.jpg")).toBeNull();
});
it("folder filter excludes a neighboring folder",async()=>{
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="saved_library")return {photos:[{...scan(A).photos[0],path:A+"\\Trips\\one.jpg",name:"one.jpg"},{...scan(A).photos[0],path:A+"\\Trips-old\\other.jpg",name:"other.jpg"}],scanned_files:2,truncated:false};
  return fallback(command,args);
 });
 localStorage.setItem("openphoto.workspaces",JSON.stringify(workspace([A])));
 render(<App/>);await screen.findByText("other.jpg");openFolders();
 fireEvent.click(screen.getByRole("button",{name:"Trips"}));
 expect(screen.queryByText("other.jpg")).toBeNull();
});
