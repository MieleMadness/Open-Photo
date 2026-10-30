import {StrictMode} from "react";
import {act,cleanup,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {useVideoElement} from "./mediaPlayback";
afterEach(async()=>{cleanup();await act(async()=>{});vi.restoreAllMocks();});
function Player(){const [,bind]=useVideoElement();return <video aria-label="Test player" ref={bind} src="local-preview.mp4"/>;}
it("preserves an attached source through StrictMode ref reattachment and releases it after closing",async()=>{
  const pause=vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{}),load=vi.spyOn(HTMLMediaElement.prototype,"load").mockImplementation(()=>{});
  const ui=render(<StrictMode><Player/></StrictMode>),video=screen.getByLabelText("Test player");
  await act(async()=>{});expect(video.getAttribute("src")).toBe("local-preview.mp4");expect(pause).not.toHaveBeenCalled();
  ui.unmount();await waitFor(()=>expect(video.getAttribute("src")).toBeNull());expect(pause).toHaveBeenCalledOnce();expect(load).toHaveBeenCalledOnce();
});
