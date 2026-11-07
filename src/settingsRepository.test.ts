import {afterEach,expect,it,vi} from "vitest";
import {readStructuredSetting,validateStructuredSetting,repairStructuredSetting} from "./settingsRepository";
import {readSetting,saveSetting,flushSettings} from "./userData";
vi.mock("./userData",()=>({readSetting:vi.fn(),saveSetting:vi.fn(),flushSettings:vi.fn(async()=>{})}));
afterEach(()=>vi.clearAllMocks());
it.each(["{}","null","[null]",'[{"id":"one","name":"Library"}]','{"schemaVersion":99,"value":[]}'])("preserves and rejects malformed or future workspace data: %s",raw=>{
  vi.mocked(readSetting).mockReturnValue(raw);expect(()=>readStructuredSetting("workspaces")).toThrow(/preserved/);expect(saveSetting).not.toHaveBeenCalled();
});
it("reads legacy arrays and supported v1 envelopes without writing",()=>{
  const value=[{id:"one",name:"Library",root:"C:/Photos"}];expect(validateStructuredSetting("workspaces",JSON.stringify(value))).toEqual(value);expect(validateStructuredSetting("workspaces",JSON.stringify({schemaVersion:1,value}))).toEqual(value);expect(saveSetting).not.toHaveBeenCalled();
});
it("writes only an explicit valid repair and awaits durable storage",async()=>{
  await expect(repairStructuredSetting("collections","{}")).rejects.toThrow();expect(saveSetting).not.toHaveBeenCalled();
  await repairStructuredSetting("collections","[]");expect(saveSetting).toHaveBeenCalledWith("openphoto.collections","[]");expect(flushSettings).toHaveBeenCalledWith(["openphoto.collections"]);
});
