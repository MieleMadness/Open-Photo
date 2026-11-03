import {StrictMode} from "react";
import {act,cleanup,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {savedVideoVolume,saveVideoVolume,useVideoElement} from "./mediaPlayback";
afterEach(async()=>{cleanup();localStorage.clear();await act(async()=>{});vi.restoreAllMocks();});
function Player(){const [,bind]=useVideoElement();return <video aria-label="Test player" ref={bind} src="local-preview.mp4"/>;}
it("preserves an attached source through StrictMode ref reattachment and releases it after closing",async()=>{
  const pause=vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{}),load=vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  const ui=render(<StrictMode><Player/></StrictMode>),video=screen.getByLabelText("Test player");
  await act(async()=>{});expect(video.getAttribute("src")).toBe("local-preview.mp4");expect(pause).not.toHaveBeenCalled();
  ui.unmount();await waitFor(()=>expect(video.getAttribute("src")).toBeNull());expect(pause).toHaveBeenCalledOnce();expect(load).toHaveBeenCalledOnce();
});

it("restores the global player volume on a newly mounted video and saves later changes",()=>{
  vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  localStorage.setItem("openphoto.videoVolume","0.35");
  render(<Player/>);
  expect((screen.getByLabelText("Test player") as HTMLVideoElement).volume).toBeCloseTo(.35);
  saveVideoVolume(.62);
  expect(localStorage.getItem("openphoto.videoVolume")).toBe("0.62");
});

it("defaults fresh playback to full volume while preserving an explicitly saved zero",()=>{
  localStorage.removeItem("openphoto.videoVolume");expect(savedVideoVolume()).toBe(1);
  localStorage.setItem("openphoto.videoVolume","0");expect(savedVideoVolume()).toBe(0);
});
