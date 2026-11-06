import {beforeEach,afterEach,it,expect,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
vi.mock("./videoExport",()=>({hasStartingVideoExport:()=>false,refreshVideoExport:async()=>null,setAppClosing:vi.fn()}));
vi.mock("./videoProject",()=>({discardProjectSaves:vi.fn(),hasUnsavedProjects:()=>false,flushProjects:vi.fn()}));
beforeEach(()=>{vi.resetModules();vi.mocked(invoke).mockReset();localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});});
afterEach(()=>{delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__});
it.each([false,true])("waits for queued settings before exit, project discard=%s",async discard=>{
  let finish:()=>void=()=>{};
  vi.mocked(invoke).mockImplementation(()=>new Promise<void>(resolve=>{finish=resolve}));
  const {saveSetting}=await import("./userData");
  const {withAppExitProtection}=await import("./appExit");
  saveSetting("openphoto.ratings",'{"photo":5}');
  const action=vi.fn(async()=>{});const closing=withAppExitProtection(action,discard);
  await vi.waitFor(()=>expect(invoke).toHaveBeenCalled());
  expect(action).not.toHaveBeenCalled();
  expect(()=>saveSetting("openphoto.ratings","{}")).toThrow("app is closing");
  expect(localStorage.getItem("openphoto.ratings")).toBe('{"photo":5}');
  finish();await closing;expect(action).toHaveBeenCalledOnce();
});
it("blocks discard on settings failure, permits retry, and restores durable values on restart",async()=>{
  const durable:Record<string,string>={};let failed=true;
  vi.mocked(invoke).mockImplementation(async(command,args)=>{
    if(command==="load_user_data")return {...durable};
    if(failed)throw Error("Disk full");
    const {key,value}=args as {key:string;value:string};durable[key]=value;
  });
  const {saveSetting,retrySettings}=await import("./userData");
  const {withAppExitProtection}=await import("./appExit");
  saveSetting("openphoto.ratings",'{"photo":5}');const action=vi.fn(async()=>{});
  await expect(withAppExitProtection(action,true)).rejects.toThrow("Disk full");expect(action).not.toHaveBeenCalled();
  failed=false;await retrySettings();await withAppExitProtection(action);expect(action).toHaveBeenCalledOnce();
  vi.resetModules();localStorage.clear();const {hydrateSettings,readSetting}=await import("./userData");await hydrateSettings();
  expect(readSetting("openphoto.ratings")).toBe('{"photo":5}');
});
