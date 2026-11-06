import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {Update} from "@tauri-apps/plugin-updater";
import {checkForAppUpdate} from "./appUpdateService";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),Resource:class{constructor(public rid:number){}}}));
afterEach(()=>vi.resetAllMocks());
it("preserves the native update resource and metadata in Tauri's SDK handle",async()=>{
  vi.mocked(invoke).mockResolvedValue({rid:42,currentVersion:"0.56.2",version:"1.0.0",date:"2026-10-29T00:00:00Z",body:"Release notes",rawJson:{version:"1.0.0"}});
  const candidate=await checkForAppUpdate();
  expect(invoke).toHaveBeenCalledWith("check_app_update");expect(candidate).toBeInstanceOf(Update);
  expect(candidate?.rid).toBe(42);expect(candidate?.version).toBe("1.0.0");expect(candidate?.body).toBe("Release notes");
});
it("returns no resource when the native channel has no update",async()=>{
  vi.mocked(invoke).mockResolvedValue(null);expect(await checkForAppUpdate()).toBeNull();
});
