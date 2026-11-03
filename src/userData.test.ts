import {beforeEach,afterEach,describe,it,expect,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
beforeEach(()=>{
  vi.resetModules();vi.mocked(invoke).mockReset();localStorage.clear();
  Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});
});
afterEach(()=>{delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__});

describe("durable metadata",()=>{
  it("waits for workspace ownership to save and surfaces its failure before project access",async()=>{
    let finish:()=>void=()=>{};
    vi.mocked(invoke).mockImplementation(()=>new Promise<void>(resolve=>{finish=resolve;}));
    const {saveSetting,flushSettings}=await import("./userData");saveSetting("openphoto.workspaces","[]");
    let complete=false;const wait=flushSettings(["openphoto.workspaces"]).then(()=>{complete=true;});
    await Promise.resolve();await Promise.resolve();expect(complete).toBe(false);finish();await wait;expect(complete).toBe(true);
    vi.mocked(invoke).mockRejectedValue(Error("Workspace storage unavailable"));saveSetting("openphoto.workspaces","[]");
    await expect(flushSettings(["openphoto.workspaces"])).rejects.toThrow("Workspace storage unavailable");
  });
  it("hydrates durable values before the app starts, retaining legacy values for migration",async()=>{
    localStorage.setItem("openphoto.ratings",'{"photo":2}');
    vi.mocked(invoke).mockResolvedValue({"openphoto.ratings":'{"photo":5}'});
    const {hydrateSettings}=await import("./userData");await hydrateSettings();
    expect(invoke).toHaveBeenCalledWith("load_user_data",{legacy:{"openphoto.ratings":'{"photo":2}'}});
    expect(localStorage.getItem("openphoto.ratings")).toBe('{"photo":5}');
  });
  it("does not replace local metadata when durable loading fails",async()=>{
    localStorage.setItem("openphoto.ratings",'{"photo":2}');
    vi.mocked(invoke).mockRejectedValue(Error("Database unavailable"));
    const {hydrateSettings}=await import("./userData");
    await expect(hydrateSettings()).rejects.toThrow("Database unavailable");
    expect(localStorage.getItem("openphoto.ratings")).toBe('{"photo":2}');
  });
  it("reports any failed setting even if the last queued save succeeds",async()=>{
    localStorage.setItem("openphoto.ratings","{}");localStorage.setItem("openphoto.theme","dark");
    vi.mocked(invoke).mockImplementation(async(_command,args)=>{
      if((args as {key:string}).key==="openphoto.ratings")throw Error("Disk full");
      return undefined;
    });
    const {retrySettings}=await import("./userData");
    await expect(retrySettings()).rejects.toThrow("Disk full");
    vi.mocked(invoke).mockResolvedValue(undefined);
    await expect(retrySettings()).resolves.toBeUndefined();
  });
});
