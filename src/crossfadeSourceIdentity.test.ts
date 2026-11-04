import {describe,expect,it} from "vitest";
import {captureCrossfadeSourceIdentity,requireCrossfadeSourceIdentity,type CrossfadeSourceClip} from "./crossfadeSourceIdentity";

const source=(id="clip-a"):CrossfadeSourceClip=>({
  clip_id:id,asset_id:"asset-a",sha256:"AB".repeat(32),bytes:1048576,
  source_in:{ticks:"1000000",time_base:"1/1000000"},
  source_out:{ticks:"2000000",time_base:"1/1000000"},
});
describe("experimental Phase J source binding",()=>{
  it("captures ordered source fingerprints and normalizes hex casing",()=>{
    const clip=source();
    const result=captureCrossfadeSourceIdentity([clip]);
    expect(result.version).toBe(1);
    expect(result.clips[0].sha256).toBe("ab".repeat(32));
    clip.source_in.ticks="999999";
    expect(result.clips[0].source_in.ticks).toBe("1000000");
    expect(()=>requireCrossfadeSourceIdentity(result,[source()])).not.toThrow();
  });
  it("rejects changed content, size, source or trim even with unchanged frame count",()=>{
    const saved=captureCrossfadeSourceIdentity([source()]);
    const candidates=[
      {...source(),sha256:"cd".repeat(32)},
      {...source(),bytes:1048577},
      {...source(),asset_id:"asset-b"},
      {...source(),source_in:{ticks:"1000001",time_base:"1/1000000"}},
      {...source(),source_out:{ticks:"2000001",time_base:"1/1000000"}},
    ];
    for(const clip of candidates)
      expect(()=>requireCrossfadeSourceIdentity(saved,[clip])).toThrow(/no longer matches/);
  });
  it("keeps clip order significant while allowing repeated assets with separate clips",()=>{
    const first=source("clip-a"),second=source("clip-b");
    const saved=captureCrossfadeSourceIdentity([first,second]);
    expect(()=>requireCrossfadeSourceIdentity(saved,[first,second])).not.toThrow();
    expect(()=>requireCrossfadeSourceIdentity(saved,[second,first])).toThrow(/no longer matches/);
    expect(()=>captureCrossfadeSourceIdentity([first,first])).toThrow(/Duplicate/);
  });
  it("compares exact rational trims without lossy floating point time conversion",()=>{
    const saved=captureCrossfadeSourceIdentity([source()]);
    const equivalent={...source(),source_in:{ticks:"1000",time_base:"1/1000"}};
    expect(()=>requireCrossfadeSourceIdentity(saved,[equivalent])).toThrow(/no longer matches/);
    expect(()=>captureCrossfadeSourceIdentity([{
      ...source(),source_in:{ticks:"1",time_base:"1/3"},source_out:{ticks:"2",time_base:"1/3"}
    }])).not.toThrow();
  });
  it("rejects invalid, negative, reversed and oversized trim marks",()=>{
    const variants=[
      {...source(),source_in:{ticks:"-1",time_base:"1/1000000"}},
      {...source(),source_out:{ticks:"0",time_base:"1/1000000"}},
      {...source(),source_out:{ticks:"123456789012345678901",time_base:"1/1000000"}},
      {...source(),source_in:{ticks:"1",time_base:"0/1"}},
      {...source(),source_out:{ticks:"2",time_base:"1/0"}},
      {...source(),bytes:0},
      {...source(),sha256:"short"},
    ];
    for(const item of variants)expect(()=>captureCrossfadeSourceIdentity([item])).toThrow(/Invalid/);
  });
  it("rejects unsupported or corrupt saved identity records",()=>{
    const saved=captureCrossfadeSourceIdentity([source()]);
    expect(()=>requireCrossfadeSourceIdentity({...saved,version:2 as 1},[source()])).toThrow(/Unsupported/);
    expect(()=>requireCrossfadeSourceIdentity({...saved,clips:null as unknown as CrossfadeSourceClip[]},[source()])).toThrow(/Unsupported/);
    expect(()=>captureCrossfadeSourceIdentity(Array.from({length:201},(_,i)=>source("clip-"+i)))).toThrow(/limit/);
  });
});
