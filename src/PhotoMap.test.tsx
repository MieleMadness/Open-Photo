import {act,cleanup,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
const mocks=vi.hoisted(()=>({map:vi.fn(),invalidateSize:vi.fn(),remove:vi.fn(),disconnect:vi.fn(),observe:vi.fn()}));
vi.mock("leaflet",()=>({default:{map:mocks.map,tileLayer:()=>({addTo:vi.fn()})}}));
import PhotoMap from "./PhotoMap";
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();vi.clearAllMocks();});
it("resizes the existing map after sidebar changes and cleans up pending work",()=>{
  vi.useFakeTimers();let resized:()=>void=()=>{};
  vi.stubGlobal("ResizeObserver",class {constructor(callback:()=>void){resized=callback;}observe=mocks.observe;disconnect=mocks.disconnect;});
  const map={setView:vi.fn().mockReturnThis(),invalidateSize:mocks.invalidateSize,remove:mocks.remove};mocks.map.mockReturnValue(map);
  const {unmount}=render(<PhotoMap points={[]} onChoose={()=>{}}/>);
  expect(map.setView).toHaveBeenCalledExactlyOnceWith([27.95,-82.46],6);
  act(()=>resized());expect(mocks.invalidateSize).toHaveBeenCalledWith({pan:false});expect(mocks.map).toHaveBeenCalledOnce();
  unmount();expect(mocks.disconnect).toHaveBeenCalledOnce();expect(mocks.remove).toHaveBeenCalledOnce();
  mocks.invalidateSize.mockClear();act(()=>vi.runAllTimers());expect(mocks.invalidateSize).not.toHaveBeenCalled();
});
