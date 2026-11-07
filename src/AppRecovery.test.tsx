import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {AppRecovery} from "./AppRecovery";
import {readStructuredSetting} from "./settingsRepository";
import {readSetting,saveSetting,flushSettings} from "./userData";
vi.mock("./userData",()=>({readSetting:vi.fn(),saveSetting:vi.fn(),flushSettings:vi.fn(async()=>{})}));
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.clearAllMocks();});
it("blocks rendering without overwriting malformed data and reopens only after a validated repair",async()=>{
  vi.spyOn(console,"error").mockImplementation(()=>{});
  let raw="{}";vi.mocked(readSetting).mockImplementation(()=>raw);vi.mocked(saveSetting).mockImplementation((_key,value)=>{raw=value;});
  function Library(){return <p>{readStructuredSetting("workspaces").length} workspaces opened</p>;}
  render(<AppRecovery><Library/></AppRecovery>);
  expect(screen.getByRole("heading",{name:"Repair saved workspaces"})).toBeTruthy();expect(saveSetting).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Saved settings JSON"),{target:{value:"null"}});fireEvent.click(screen.getByRole("button",{name:"Apply repaired settings"}));
  await waitFor(()=>expect(screen.getByText(/unexpected data shape/)).toBeTruthy());expect(saveSetting).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Saved settings JSON"),{target:{value:"[]"}});fireEvent.click(screen.getByRole("button",{name:"Apply repaired settings"}));
  expect(await screen.findByText("0 workspaces opened")).toBeTruthy();expect(flushSettings).toHaveBeenCalledWith(["openphoto.workspaces"]);
});
