import {act,cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {EditorPoster} from "./EditorPoster";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.unstubAllGlobals();vi.resetAllMocks();});
it("limits native poster work, skips queued unmounted items and reuses verified identities",async()=>{
  vi.stubGlobal("IntersectionObserver",undefined);
  const finish:((image:string)=>void)[]=[];
  vi.mocked(invoke).mockImplementation(()=>new Promise<string>(resolve=>finish.push(resolve)));
  const ui=render(<>{[0,1,2,3].map(i=><EditorPoster key={i} root="/media" path={`/media/${i}.mp4`} identity={String(i)}/>)}</>);
  expect(invoke).toHaveBeenCalledTimes(2);ui.unmount();
  await act(async()=>{finish.forEach(resolve=>resolve("data:image/png;base64,poster"));});
  expect(invoke).toHaveBeenCalledTimes(2);
  const cached=render(<EditorPoster root="/media" path="/media/0.mp4" identity="0"/>);
  await act(async()=>{});expect(cached.container.querySelector("img")?.getAttribute("src")).toBe("data:image/png;base64,poster");
  expect(invoke).toHaveBeenCalledTimes(2);expect(screen.queryByText("Loading")).toBeNull();
});
