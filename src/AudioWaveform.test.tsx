import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {AudioWaveform} from "./AudioWaveform";

vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

it("shows measured waveform peaks and seeks on the global project timeline",async()=>{
  vi.mocked(invoke).mockResolvedValue(Array.from({length:64},(_,index)=>index%2?45:80));
  const seek=vi.fn();
  render(<AudioWaveform workspaceId="workspace" projectId="project" assetId="sound"
    revision={2} sourceIn={3} duration={20} trackStart={10} playhead={12}
    onSeek={seek} ready/>);
  const slider=await screen.findByRole("slider",{name:"Seek project using audio waveform"});
  await waitFor(()=>expect(document.querySelectorAll(".sequence-waveform-peak")).toHaveLength(64));
  expect((slider as HTMLInputElement).value).toBe("2");
  fireEvent.change(slider,{target:{value:"4.25"}});
  expect(seek).toHaveBeenCalledWith(14.25);
  expect(invoke).toHaveBeenCalledWith("workspace_audio_waveform",{
    workspaceId:"workspace",id:"project",assetId:"sound",expectedRevision:2,
    sourceInSeconds:3,durationSeconds:20
  });
});
it("does not request inaccessible audio files",()=>{
  render(<AudioWaveform workspaceId="workspace" projectId="project" assetId="sound"
    revision={2} sourceIn={0} duration={10} trackStart={0} playhead={0}
    onSeek={vi.fn()} ready={false}/>);
  expect(screen.getByText("Relink this file to view its waveform.")).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});

it.each([
  ["wrong count",Array.from({length:63},()=>35)],
  ["non-finite peak",Array.from({length:64},(_,index)=>index===12?Number.NaN:35)],
  ["negative peak",Array.from({length:64},(_,index)=>index===12?-1:35)],
  ["peak above 100",Array.from({length:64},(_,index)=>index===12?101:35)],
  ["non-array response",null],
])("does not render %s from native waveform measurements",async(_caseName,invalidPeaks)=>{
  vi.mocked(invoke).mockResolvedValue(invalidPeaks);
  render(<AudioWaveform workspaceId="workspace" projectId="project" assetId="sound"
    revision={2} sourceIn={0} duration={10} trackStart={0} playhead={0}
    onSeek={vi.fn()} ready/>);
  expect(await screen.findByText("Waveform unavailable — audio editing still works.")).toBeTruthy();
  expect(screen.queryByRole("slider")).toBeNull();
  expect(document.querySelectorAll(".sequence-waveform-peak")).toHaveLength(0);
});

it("ignores a stale native response after the audio source changes",async()=>{
  let completeFirst:(value:number[])=>void=()=>{};
  vi.mocked(invoke).mockReturnValueOnce(new Promise<number[]>(resolve=>{completeFirst=resolve;}))
    .mockResolvedValueOnce(Array.from({length:64},()=>80));
  const props={workspaceId:"workspace",projectId:"project",revision:2,sourceIn:0,duration:10,trackStart:0,playhead:0,onSeek:vi.fn(),ready:true};
  const {rerender}=render(<AudioWaveform {...props} assetId="first"/>);
  rerender(<AudioWaveform {...props} assetId="second"/>);
  await screen.findByRole("slider");
  completeFirst(Array.from({length:63},()=>40));
  await waitFor(()=>expect(screen.queryByText("Waveform unavailable — audio editing still works.")).toBeNull());
  expect(document.querySelectorAll(".sequence-waveform-peak")).toHaveLength(64);
});
