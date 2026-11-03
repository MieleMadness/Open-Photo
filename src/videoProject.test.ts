import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {cropForAspect,sourceCanvas,defaultEdits,validProject,ProjectSession,hasUnsavedProjects,flushProjects,discardProjectSaves,type VideoProject} from "./videoProject";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
function project():VideoProject{return {schema_version:1,id:"p",title:"Saved project",asset:{id:"asset",relative_path:"source.mp4",sha256:"0".repeat(64),bytes:123,info:{duration_seconds:10,codec:"h264",width:1920,height:1080,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]}},clips:[{id:"clip",asset_id:"asset",source_in:{ticks:"0",time_base:"1/90000"},source_out:{ticks:"900000",time_base:"1/90000"},edits:defaultEdits()}],output:{width:1920,height:1080,fit:"fit"}};}
afterEach(()=>{discardProjectSaves();vi.resetAllMocks();});
it("rejects malformed brightness/contrast before autosave and accepts the adjustment endpoints",()=>{
  for(const field of ["brightness","contrast"] as const){
    for(const value of [-101,101,0.5,NaN,Infinity]){
      const p=project();p.clips[0].edits[field]=value;
      expect(validProject(p)).toMatch(/Brightness and contrast/);
    }
    for(const value of [-100,0,100]){
      const p=project();p.clips[0].edits[field]=value;expect(validProject(p)).toBe("");
    }
  }
});
it("serializes revisions, persists the latest edit after unmount, and makes undo/redo durable",async()=>{
  let finish:(revision:number)=>void=()=>{};
  vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve})).mockResolvedValueOnce(3).mockResolvedValueOnce(4).mockResolvedValueOnce(5);
  const p=project();const s=new ProjectSession("root",{project:p,revision:1,source_path:"source.mp4",source_status:"ready"});
  const first=structuredClone(p);first.clips[0].edits.quarter_turns=1;s.commit(first);
  const next=structuredClone(first);next.clips[0].edits.muted=true;s.commit(next);s.release();expect(invoke).toHaveBeenCalledTimes(1);
  finish(2);await s.flush();expect(invoke).toHaveBeenNthCalledWith(2,"save_video_project",{root:"root",project:next,expectedRevision:2});
  s.commit("undo");await s.flush();expect(s.history.present).toEqual(first);
  s.commit("redo");await s.flush();expect(s.history.present).toEqual(next);expect(hasUnsavedProjects()).toBe(false);
});
it("keeps failed edits recoverable and does not advance the expected revision until success",async()=>{
  vi.mocked(invoke).mockRejectedValueOnce(Error("Disk full")).mockResolvedValueOnce(8);
  const p=project();const s=new ProjectSession("root",{project:p,revision:7,source_path:null,source_status:"missing"});
  const next=structuredClone(p);next.clips[0].edits.gain_percent=50;s.commit(next);
  await expect(s.flush()).rejects.toThrow("Disk full");expect(hasUnsavedProjects()).toBe(true);expect(s.history.present).toEqual(next);
  await flushProjects(true);expect(invoke).toHaveBeenNthCalledWith(2,"save_video_project",{root:"root",project:next,expectedRevision:7});expect(s.revision).toBe(8);
});
it("validates trim/crop changes before mutating history and preserves the original project",()=>{
  const p=project();const s=new ProjectSession("root",{project:p,revision:1,source_path:null,source_status:"missing"});
  const bad=structuredClone(p);bad.clips[0].source_out.ticks="0";expect(()=>s.commit(bad)).toThrow();expect(s.history.present).toBe(p);expect(invoke).not.toHaveBeenCalled();
  const square=cropForAspect(p.asset.info,1);expect(square).toEqual({x:2187,y:0,width:5625,height:10000});
  expect(cropForAspect({...p.asset.info,rotation_degrees:90},1).height).toBe(5625);
});

it("keeps source canvas geometry for UHD, portrait, ultra-wide and anamorphic clips",()=>{
  const info=project().asset.info;
  expect(sourceCanvas({...info,width:3840,height:2160})).toEqual({width:3840,height:2160,fit:"fit"});
  expect(sourceCanvas({...info,width:3840,height:2160,rotation_degrees:90})).toEqual({width:2160,height:3840,fit:"fit"});
  expect(sourceCanvas({...info,width:3840,height:1080})).toEqual({width:3840,height:1080,fit:"fit"});
  expect(sourceCanvas({...info,width:1440,height:1080,sample_aspect_ratio:"4:3"})).toEqual({width:1920,height:1080,fit:"fit"});
  expect(sourceCanvas({...info,width:7680,height:4320})).toBeNull();
});
