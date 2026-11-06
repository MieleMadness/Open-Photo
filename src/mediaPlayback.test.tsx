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

it("updates an already mounted player when another player changes global volume",()=>{
  vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});
  vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  render(<Player/>);
  const video=screen.getByLabelText("Test player") as HTMLVideoElement;
  expect(video.volume).toBe(1);
  act(()=>saveVideoVolume(0.27));
  expect(video.volume).toBeCloseTo(0.27);
  expect(localStorage.getItem("openphoto.videoVolume")).toBe("0.27");
});

it("rejects invalid volume input without corrupting the saved preference",()=>{
  localStorage.setItem("openphoto.videoVolume","0.4");
  saveVideoVolume(NaN);
  saveVideoVolume(Infinity);
  expect(savedVideoVolume()).toBeCloseTo(0.4);
  expect(localStorage.getItem("openphoto.videoVolume")).toBe("0.4");
  saveVideoVolume(2);
  expect(savedVideoVolume()).toBe(1);
  saveVideoVolume(-1);
  expect(savedVideoVolume()).toBe(0);
});
