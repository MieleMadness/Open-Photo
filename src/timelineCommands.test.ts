import {expect,it} from "vitest";
import {defaultEdits} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
import {editTimeline,projectDuration,timelineEntries} from "./timelineCommands";
import {validSequence,type SequenceProject} from "./videoSequence";

function project():SequenceProject{
  const info={duration_seconds:2,codec:"h264",width:96,height:64,time_base:"1/90000",frame_rate:"30/1",nominal_frame_rate:"30/1",sample_aspect_ratio:"1:1",rotation_degrees:0,color_transfer:null,hdr:false,audio:[],warnings:[]};
  return {schema_version:2,id:"p",title:"Short assembly",assets:["one","two"].map((id,i)=>({id,relative_path:`${id}.mp4`,sha256:String(i+1).repeat(64),bytes:100,info})),clips:["one","two"].map(id=>({id,asset_id:id,source_in:markTime(0,info.time_base),source_out:markTime(2,info.time_base),edits:defaultEdits()})),output:{width:1920,height:1080,fit:"fit"},frame_rate:30};
}
it("splits at project time into adjacent source ticks while preserving edits and linked audio",()=>{
  const p=project();p.clips[1].edits.gain_percent=75;p.clips[1].edits.brightness=20;
  const before=structuredClone(p),next=editTimeline(p,{type:"split",clipId:"two",at:2+1/3,newId:"right"});
  expect(next.clips[1].source_out).toEqual(next.clips[2].source_in);
  expect(markSeconds(next.clips[2].source_in)).toBeCloseTo(1/3,5);
  expect(next.clips[2].edits).toEqual(p.clips[1].edits);expect(next.clips[2].asset_id).toBe("two");
  expect(projectDuration(next)).toBe(4);expect(p).toEqual(before);
});
it("reorders whole clips and ripple deletion derives a gapless placement without changing source assets",()=>{
  const p=project(),moved=editTimeline(p,{type:"move",clipId:"two",index:0});
  expect(moved.clips.map(c=>c.id)).toEqual(["two","one"]);
  const trimmed=editTimeline(moved,{type:"trim",clipId:"two",start:0.5,end:1.5});
  expect(timelineEntries(trimmed).map(e=>[e.start,e.end])).toEqual([[0,1],[1,3]]);
  const deleted=editTimeline(trimmed,{type:"delete",clipId:"two"});
  expect(timelineEntries(deleted).map(e=>[e.start,e.end])).toEqual([[0,2]]);expect(deleted.assets).toEqual(p.assets);
});
it("allows an empty durable timeline and repeated instances of an asset",()=>{
  let p=project();for(const id of ["one","two"])p=editTimeline(p,{type:"delete",clipId:id});
  expect(validSequence(p)).toBe("");expect(projectDuration(p)).toBe(0);
  p=editTimeline(p,{type:"append",assetId:"one",newId:"a"});p=editTimeline(p,{type:"append",assetId:"one",newId:"b"});
  expect(p.clips.map(c=>c.asset_id)).toEqual(["one","one"]);expect(p.assets).toHaveLength(2);
});
it("rejects invalid edits atomically including tiny splits, source overflow and duplicate identities",()=>{
  const p=project(),before=structuredClone(p);
  for(const action of [
    {type:"split",clipId:"one",at:0.01,newId:"right"},
    {type:"split",clipId:"one",at:1,newId:"two"},
    {type:"trim",clipId:"one",start:-1,end:1},
    {type:"trim",clipId:"one",start:0,end:3},
    {type:"move",clipId:"one",index:2},
    {type:"append",assetId:"missing",newId:"x"},
  ] as const){expect(()=>editTimeline(p,action)).toThrow();expect(p).toEqual(before);}
  p.clips=[];p.title=" ";expect(validSequence(p)).not.toBe("");
});
