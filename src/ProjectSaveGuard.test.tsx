import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {flushProjects} from "./videoProject";
import {ProjectSaveGuard} from "./ProjectSaveGuard";
import {refreshVideoExport,hasStartingVideoExport,type ExportStatus} from "./videoExport";
vi.mock("./videoExport",()=>({refreshVideoExport:vi.fn(),hasStartingVideoExport:vi.fn(),setAppClosing:vi.fn()}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:vi.fn()}));
vi.mock("./videoProject",()=>({hasUnsavedProjects:()=>true,flushProjects:vi.fn(),discardProjectSaves:vi.fn(),projectSaveError:()=>"",subscribeProjectSaves:()=>()=>{}}));
afterEach(()=>{cleanup();delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;vi.resetAllMocks();});
beforeEach(()=>{vi.mocked(refreshVideoExport).mockResolvedValue(null);vi.mocked(hasStartingVideoExport).mockReturnValue(false);});
async function setup(){
  (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__={};
  let callback:(event:{preventDefault:()=>void})=>Promise<void>=async()=>{};const close=vi.fn().mockResolvedValue(undefined);
  vi.mocked(getCurrentWindow).mockReturnValue({onCloseRequested:async(handler:typeof callback)=>{callback=handler;return()=>{};},close,destroy:close} as unknown as ReturnType<typeof getCurrentWindow>);
  render(<ProjectSaveGuard/>);await act(async()=>{});return{close,request:()=>callback({preventDefault:vi.fn()})};
}
it("waits for queued project writes before allowing window closure",async()=>{
  let finish:()=>void=()=>{};vi.mocked(flushProjects).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
  const {close,request}=await setup();let pending:Promise<void>=Promise.resolve();
  await act(async()=>{pending=request();await Promise.resolve();});expect(close).not.toHaveBeenCalled();await act(async()=>{finish();await pending;});expect(close).toHaveBeenCalledOnce();
});
it("keeps the app open until a running export finishes or is cancelled",async()=>{
  vi.mocked(refreshVideoExport).mockResolvedValue({active:true} as ExportStatus);
  const {close,request}=await setup();await act(async()=>request());expect(close).not.toHaveBeenCalled();expect(flushProjects).not.toHaveBeenCalled();expect(screen.getByText(/An export is still running/)).toBeTruthy();
});
it("blocks close while an export start is awaiting native registration",async()=>{
  vi.mocked(hasStartingVideoExport).mockReturnValue(true);
  const {close,request}=await setup();await act(async()=>request());expect(close).not.toHaveBeenCalled();expect(screen.getByText(/An export is still running or starting/)).toBeTruthy();
});
it("keeps the window open when saving fails and allows an explicit retry",async()=>{
  vi.mocked(flushProjects).mockRejectedValueOnce(Error("Disk full")).mockResolvedValueOnce(undefined);
  const {close,request}=await setup();await act(async()=>request());expect(close).not.toHaveBeenCalled();expect(screen.getByRole("alert").textContent).toContain("Disk full");
  fireEvent.click(screen.getByText("Retry project saves"));await act(async()=>{});expect(flushProjects).toHaveBeenLastCalledWith(true);
});
