import {expect,it,vi} from "vitest";
import {PreviewService,type PreviewRequest} from "./previewService";
const request:PreviewRequest={root:"C:/Media",path:"C:/Media/one.jpg",sourceVersion:"one",bytes:1,modified:1,maxEdge:320,kind:"image"};
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
it("deduplicates subscribers, caches only versioned results, and partitions changed identities",async()=>{
  const load=vi.fn(async(r:PreviewRequest)=>r.sourceVersion||"legacy"),service=new PreviewService(load),first=vi.fn(),second=vi.fn();
  service.subscribe(request,first);service.subscribe(request,second);await tick();expect(load).toHaveBeenCalledTimes(1);expect(first).toHaveBeenCalledWith("one");expect(second).toHaveBeenCalledWith("one");
  service.subscribe(request,vi.fn());expect(load).toHaveBeenCalledTimes(1);
  service.subscribe({...request,sourceVersion:"two"},vi.fn());await tick();expect(load).toHaveBeenCalledTimes(2);
  service.subscribe({...request,sourceVersion:null},vi.fn());await tick();service.subscribe({...request,sourceVersion:null},vi.fn());await tick();expect(load).toHaveBeenCalledTimes(4);
});
it("bounds all previews to three active jobs and HEIC to one, retaining canceled slots until completion",async()=>{
  const active:{r:PreviewRequest;signal:AbortSignal;finish:(value:string)=>void}[]=[];
  const service=new PreviewService((r,signal)=>new Promise(resolve=>active.push({r,signal,finish:resolve}))),notify=vi.fn();
  const cancel=service.subscribe({...request,kind:"heic",path:"one.heic"},notify);
  service.subscribe({...request,kind:"heic",path:"two.heic"},vi.fn());
  for(let i=0;i<3;i++)service.subscribe({...request,path:`${i}.jpg`},vi.fn());
  expect(active).toHaveLength(3);expect(active.filter(item=>item.r.kind==="heic")).toHaveLength(1);
  cancel();expect(active[0].signal.aborted).toBe(true);expect(active).toHaveLength(3);
  active[0].finish("old");await tick();expect(notify).not.toHaveBeenCalled();expect(active).toHaveLength(4);
  for(const job of active)job.finish("preview");await tick();for(const job of active)job.finish("preview");await tick();
});
