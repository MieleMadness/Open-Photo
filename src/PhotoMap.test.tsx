import {act,cleanup,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
const mocks=vi.hoisted(()=>({map:vi.fn(),invalidateSize:vi.fn(),remove:vi.fn(),disconnect:vi.fn(),observe:vi.fn(),circleMarker:vi.fn(),bindTooltip:vi.fn()}));
vi.mock("leaflet",()=>({default:{map:mocks.map,circleMarker:mocks.circleMarker,latLngBounds:vi.fn(points=>points),tileLayer:()=>({addTo:vi.fn()}),layerGroup:()=>({addTo:()=>({clearLayers:vi.fn()})})}}));
import PhotoMap from "./PhotoMap";
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();vi.clearAllMocks();});
it("resizes the existing map after sidebar changes and cleans up pending work",()=>{
  vi.useFakeTimers();let resized:()=>void=()=>{};
  vi.stubGlobal("ResizeObserver",class {constructor(callback:()=>void){resized=callback;}observe=mocks.observe;disconnect=mocks.disconnect;});
  const map={setView:vi.fn().mockReturnThis(),invalidateSize:mocks.invalidateSize,remove:mocks.remove,getZoom:()=>6,on:vi.fn(),off:vi.fn()};mocks.map.mockReturnValue(map);
  const {unmount}=render(<PhotoMap points={[]} onChoose={()=>{}}/>);
  expect(map.setView).toHaveBeenCalledExactlyOnceWith([0,0],2);
  act(()=>resized());expect(mocks.invalidateSize).toHaveBeenCalledWith({pan:false});expect(mocks.map).toHaveBeenCalledOnce();
  unmount();expect(mocks.disconnect).toHaveBeenCalledOnce();expect(mocks.remove).toHaveBeenCalledOnce();
  mocks.invalidateSize.mockClear();act(()=>vi.runAllTimers());expect(mocks.invalidateSize).not.toHaveBeenCalled();
});


it.each(["Family &copy;.jpg","<img src=x onerror=alert(1)>.jpg"])("renders filename %s literally in map tooltips",filename=>{
  const choose=vi.fn();let click=()=>{};
  const marker={bindTooltip:mocks.bindTooltip,on:vi.fn((_event:string,handler:()=>void)=>{click=handler;})};
  mocks.circleMarker.mockReturnValue({addTo:()=>marker});
  mocks.map.mockReturnValue({fitBounds:vi.fn(),invalidateSize:vi.fn(),remove:vi.fn(),getZoom:()=>6,on:vi.fn(),off:vi.fn()});
  const path="C:\\photos\\"+filename;
  render(<PhotoMap points={[{path,lat:40,lon:-74}]} onChoose={choose}/>);
  const label=mocks.bindTooltip.mock.calls[0][0] as HTMLElement;
  expect(label).toBeInstanceOf(HTMLElement);
  expect(label.textContent).toBe(filename);
  expect(label.childElementCount).toBe(0);
  act(()=>click());expect(choose).toHaveBeenCalledWith(path);
});
