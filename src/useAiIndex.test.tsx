import {act,cleanup,renderHook,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {useAiIndex,isAiExcluded,type IndexReport} from "./useAiIndex";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));vi.mock("./userData",()=>({flushSettings:vi.fn(async()=>{})}));
const report:IndexReport={counts:{indexed:1,pending:0,failed:0,partial:0,unsupported:0,excluded:0},folders:[],failures:[],failure_total:0,running:false,error:null,revision:1,generation:1};
const initial={workspace:"one",enabled:true,device:"cpu",snapshot:{},desktop:true,onProgress:()=>{}};
beforeEach(()=>{vi.mocked(invoke).mockImplementation(async cmd=>cmd==="ai_index_report"?report:undefined);});afterEach(()=>{cleanup();vi.resetAllMocks();});
it("pauses before waiting for settings and starts only after durable ownership is ready",async()=>{
 let finish:()=>void=()=>{};vi.mocked(flushSettings).mockImplementation(()=>new Promise(resolve=>{finish=resolve}));
 renderHook(()=>useAiIndex(initial));expect(invoke).toHaveBeenCalledWith("pause_ai_index");expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="configure_ai_index")).toBe(false);
 await act(async()=>finish());await waitFor(()=>expect(invoke).toHaveBeenCalledWith("configure_ai_index",{workspaceId:"one",enabled:true,device:"cpu"}));
});
it("pauses immediately when disabled even if saving preferences fails",async()=>{
 vi.mocked(flushSettings).mockRejectedValue(Error("Settings unavailable"));const {result}=renderHook(()=>useAiIndex({...initial,enabled:false}));await waitFor(()=>expect(result.current.error).toContain("Settings unavailable"));expect(invoke).toHaveBeenCalledWith("pause_ai_index");expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="configure_ai_index")).toBe(false);
});
it("normalizes Windows and Unicode paths without confusing neighboring folder names",()=>{
 const folders=[{path:"C:/ФОТО/Секрет",excluded:true,inherited:false,counts:report.counts}];expect(isAiExcluded("\\\\?\\c:\\фото\\секрет\\one.jpg",folders)).toBe(true);expect(isAiExcluded("C:/ФОТО/Секрет-other/one.jpg",folders)).toBe(false);
});
