import {StrictMode} from "react";
import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {getVersion} from "@tauri-apps/api/app";
import {invoke} from "@tauri-apps/api/core";
import {type Update} from "@tauri-apps/plugin-updater";
import {checkForAppUpdate as check} from "./appUpdateService";
import {useAppUpdater} from "./appUpdater";
import {flushProjects} from "./videoProject";
import {hasStartingVideoExport,refreshVideoExport,setAppClosing,type ExportStatus} from "./videoExport";
vi.mock("@tauri-apps/api/app",()=>({getVersion:vi.fn()}));
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
vi.mock("./appUpdateService",()=>({checkForAppUpdate:vi.fn()}));
vi.mock("./videoExport",()=>({refreshVideoExport:vi.fn(),hasStartingVideoExport:vi.fn(),setAppClosing:vi.fn()}));
vi.mock("./videoProject",()=>({hasUnsavedProjects:()=>true,flushProjects:vi.fn(),discardProjectSaves:vi.fn()}));
const update=()=>({version:"1.0.0",body:"Stable release",close:vi.fn().mockResolvedValue(undefined),download:vi.fn().mockResolvedValue(undefined),install:vi.fn().mockResolvedValue(undefined)}) as unknown as Update;
beforeEach(()=>{
  vi.useFakeTimers();sessionStorage.clear();
  vi.mocked(getVersion).mockResolvedValue("0.56.2");
  vi.mocked(invoke).mockResolvedValue({enabled:true,policy:"major-only"});
  vi.mocked(check).mockResolvedValue(null);
  vi.mocked(refreshVideoExport).mockResolvedValue(null);
  vi.mocked(hasStartingVideoExport).mockReturnValue(false);
  vi.mocked(flushProjects).mockResolvedValue(undefined);
});
afterEach(()=>{cleanup();vi.useRealTimers();vi.resetAllMocks();});
async function setup(){
  const hook=renderHook(()=>useAppUpdater(true),{wrapper:StrictMode});
  await act(async()=>{});return hook;
}
async function ready(){
  const candidate=update();vi.mocked(check).mockResolvedValue(candidate);
  const hook=await setup();
  await act(async()=>{await vi.advanceTimersByTimeAsync(1200);});
  await act(async()=>hook.result.current.download());
  expect(hook.result.current.phase).toBe("ready");return{...hook,candidate};
}
it("checks automatically exactly once after channel initialization under StrictMode",async()=>{
  const hook=await setup();expect(check).not.toHaveBeenCalled();
  await act(async()=>{await vi.advanceTimersByTimeAsync(1200);});
  expect(check).toHaveBeenCalledOnce();expect(hook.result.current.phase).toBe("current");
  hook.rerender();await act(async()=>{await vi.advanceTimersByTimeAsync(5000);});
  expect(check).toHaveBeenCalledOnce();
});
it("does not consume the startup check when its pending timer is unmounted",async()=>{
  const first=await setup();first.unmount();
  const second=await setup();await act(async()=>{await vi.advanceTimersByTimeAsync(1200);});
  expect(check).toHaveBeenCalledOnce();expect(second.result.current.phase).toBe("current");
});
it("closes a late update result after unmount and suppresses overlapping checks",async()=>{
  let finish:(value:Update)=>void=()=>{};
  vi.mocked(check).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
  const hook=await setup();await act(async()=>{void hook.result.current.checkNow();await Promise.resolve();});
  await act(async()=>hook.result.current.checkNow());expect(check).toHaveBeenCalledOnce();
  hook.unmount();const candidate=update();await act(async()=>finish(candidate));
  expect(candidate.close).toHaveBeenCalledOnce();
});
it("keeps a downloaded update ready when an active or starting export blocks installation",async()=>{
  const hook=await ready();
  vi.mocked(refreshVideoExport).mockResolvedValue({active:true} as ExportStatus);
  await act(async()=>hook.result.current.install());
  expect(hook.candidate.install).not.toHaveBeenCalled();expect(hook.result.current.phase).toBe("ready");
  expect(hook.result.current.message).toContain("export is still running or starting");
  vi.mocked(refreshVideoExport).mockResolvedValue(null);vi.mocked(hasStartingVideoExport).mockReturnValue(true);
  await act(async()=>hook.result.current.install());expect(hook.candidate.install).not.toHaveBeenCalled();
  expect(setAppClosing).toHaveBeenLastCalledWith(false);
});
it("waits for durable project saves before invoking the native installer",async()=>{
  const hook=await ready();let finish:()=>void=()=>{};
  vi.mocked(flushProjects).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
  let installing=Promise.resolve();await act(async()=>{installing=hook.result.current.install();await Promise.resolve();});
  expect(hook.candidate.install).not.toHaveBeenCalled();expect(setAppClosing).toHaveBeenCalledWith(true);
  await act(async()=>{finish();await installing;});expect(hook.candidate.install).toHaveBeenCalledWith({restartAfterInstall:true});
  expect(setAppClosing).toHaveBeenLastCalledWith(false);
});
it("preserves the downloaded update when a project save fails and allows retry",async()=>{
  const hook=await ready();vi.mocked(flushProjects).mockRejectedValueOnce(Error("Disk full"));
  await act(async()=>hook.result.current.install());
  expect(hook.candidate.install).not.toHaveBeenCalled();expect(hook.result.current.phase).toBe("ready");expect(hook.result.current.message).toContain("Disk full");
  await act(async()=>hook.result.current.install());expect(hook.candidate.install).toHaveBeenCalledOnce();
});
it("does not contact the public updater from a development build",async()=>{
  vi.mocked(invoke).mockResolvedValue({enabled:false,policy:"major-only"});
  const hook=await setup();await act(async()=>{await vi.advanceTimersByTimeAsync(5000);await hook.result.current.checkNow();});
  expect(check).not.toHaveBeenCalled();expect(hook.result.current.phase).toBe("disabled");
});
it("releases downloaded bytes that arrive after hook cleanup already closed the update",async()=>{
  const candidate=update();let finish:()=>void=()=>{};let bytesRetained=false;
  vi.mocked(candidate.download).mockImplementation(()=>new Promise<void>(resolve=>{finish=()=>{bytesRetained=true;resolve();};}));
  vi.mocked(candidate.close).mockImplementation(async()=>{bytesRetained=false;});
  vi.mocked(check).mockResolvedValue(candidate);const hook=await setup();
  await act(async()=>{await vi.advanceTimersByTimeAsync(1200);});
  let downloading=Promise.resolve();await act(async()=>{downloading=hook.result.current.download();await Promise.resolve();});
  hook.unmount();expect(candidate.close).toHaveBeenCalledOnce();
  await act(async()=>{finish();await downloading;});expect(bytesRetained).toBe(false);
});
