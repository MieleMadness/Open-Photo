import {beforeEach,afterEach,describe,it,expect,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
beforeEach(()=>{
  vi.resetModules();vi.mocked(invoke).mockReset();localStorage.clear();
  Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});
});
afterEach(()=>{vi.restoreAllMocks();delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__});

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
    const {hydrateSettings,readSetting}=await import("./userData");await hydrateSettings();
    expect(invoke).toHaveBeenCalledWith("load_user_data",{legacy:{"openphoto.ratings":'{"photo":2}'}});
    expect(readSetting("openphoto.ratings")).toBe('{"photo":5}');
    expect(localStorage.getItem("openphoto.ratings")).toBe('{"photo":2}');
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

describe("native memory settings",()=>{
  it("opens large native settings when browser writes fail without replacing durable values",async()=>{
    localStorage.setItem("openphoto.ratings",'{"old":1}');
    const ratings=JSON.stringify({photo:5,large:"x".repeat(6*1024*1024)});
    const durable={"openphoto.ratings":ratings,"openphoto.theme":"dark","openphoto.workspaces":'[{"id":"library"}]'};
    vi.mocked(invoke).mockResolvedValue(durable);
    const writes=vi.spyOn(Storage.prototype,"setItem").mockImplementation(()=>{throw new DOMException("Full","QuotaExceededError")});
    const {hydrateSettings,readSetting}=await import("./userData");
    await expect(hydrateSettings()).resolves.toBeUndefined();
    expect(readSetting("openphoto.ratings")).toBe(ratings);
    expect(readSetting("openphoto.workspaces")).toBe(durable["openphoto.workspaces"]);
    expect(readSetting("openphoto.theme")).toBe("dark");
    expect(writes.mock.calls.every(([key])=>key==="openphoto.theme")).toBe(true);
    expect(localStorage.getItem("openphoto.ratings")).toBe('{"old":1}');
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(durable["openphoto.ratings"]).toBe(ratings);
  });
  it("retries latest memory values after native failure even when browser storage contains older data",async()=>{
    localStorage.setItem("openphoto.ratings",'{"photo":1}');
    let fail=true;
    const durable:Record<string,string>={"openphoto.ratings":'{"photo":2}'};
    vi.mocked(invoke).mockImplementation(async(command,args)=>{
      if(command==="load_user_data")return {...durable};
      if(fail)throw Error("Disk full");
      const {key,value}=args as {key:string;value:string};durable[key]=value;
    });
    const {hydrateSettings,readSetting,saveSetting,flushSettings,retrySettings}=await import("./userData");
    await hydrateSettings();
    saveSetting("openphoto.ratings",'{"photo":5}');
    await expect(flushSettings()).rejects.toThrow("Disk full");
    expect(readSetting("openphoto.ratings")).toBe('{"photo":5}');
    expect(localStorage.getItem("openphoto.ratings")).toBe('{"photo":1}');
    fail=false;await retrySettings();
    expect(durable["openphoto.ratings"]).toBe('{"photo":5}');
    vi.resetModules();const restarted=await import("./userData");await restarted.hydrateSettings();
    expect(restarted.readSetting("openphoto.ratings")).toBe('{"photo":5}');
  });
  it("loads and saves native data when browser storage cannot even be read",async()=>{
    vi.spyOn(Storage.prototype,"getItem").mockImplementation(()=>{throw new DOMException("Disabled","SecurityError")});
    vi.spyOn(Storage.prototype,"length","get").mockImplementation(()=>{throw new DOMException("Disabled","SecurityError")});
    vi.spyOn(Storage.prototype,"key").mockImplementation(()=>{throw new DOMException("Disabled","SecurityError")});
    vi.mocked(invoke).mockImplementation(async(command)=>command==="load_user_data"?{"openphoto.ai.optin":"true"}:undefined);
    const {hydrateSettings,readSetting,saveSetting,flushSettings}=await import("./userData");
    await hydrateSettings();expect(readSetting("openphoto.ai.optin")).toBe("true");
    saveSetting("openphoto.ai.optin","false");await flushSettings();
    expect(readSetting("openphoto.ai.optin")).toBe("false");
    expect(invoke).toHaveBeenCalledWith("save_user_data",{key:"openphoto.ai.optin",value:"false"});
  });
  it("does not resurrect browser-only keys after a successful native load",async()=>{
    localStorage.setItem("openphoto.workspaces",'[{"id":"stale"}]');
    vi.mocked(invoke).mockResolvedValue({});
    const {hydrateSettings,readSetting}=await import("./userData");await hydrateSettings();
    expect(readSetting("openphoto.workspaces")).toBeNull();
  });
});
