import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
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

it("names markers, supports Enter and Space, and exposes all cluster members in the location list",()=>{
  const choose=vi.fn(),element=document.createElementNS("http://www.w3.org/2000/svg","path");
  mocks.circleMarker.mockReturnValue({addTo:()=>({bindTooltip:vi.fn(),on:vi.fn(),getElement:()=>element})});
  mocks.map.mockReturnValue({fitBounds:vi.fn(),invalidateSize:vi.fn(),remove:vi.fn(),getZoom:()=>6,on:vi.fn(),off:vi.fn()});
  render(<PhotoMap points={[{path:"C:/Photos/one.jpg",lat:40,lon:-74},{path:"C:/Photos/two.jpg",lat:40,lon:-74}]} onChoose={choose}/>);
  expect(element.getAttribute("tabindex")).toBe("0");expect(element.getAttribute("role")).toBe("button");expect(element.getAttribute("aria-label")).toContain("Browse 2");
  fireEvent.keyDown(element,{key:"Enter"});fireEvent.keyDown(element,{key:" "});
  expect(choose).toHaveBeenNthCalledWith(1,"C:/Photos/one.jpg");expect(choose).toHaveBeenNthCalledWith(2,"C:/Photos/two.jpg");
  fireEvent.click(screen.getByText("Browse locations (2)"));
  fireEvent.click(screen.getByRole("button",{name:/C:\/Photos\/two.jpg/}));expect(choose).toHaveBeenLastCalledWith("C:/Photos/two.jpg");
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
