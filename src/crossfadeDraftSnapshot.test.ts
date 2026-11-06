import {describe,expect,it} from "vitest";
import {decodeCrossfadeDraftSnapshot,encodeCrossfadeDraftSnapshot,type CrossfadeDraftSnapshotContext} from "./crossfadeDraftSnapshot";
import type {CrossfadeSourceClip} from "./crossfadeSourceIdentity";

const makeContext=():CrossfadeDraftSnapshotContext=>{
  const sources:CrossfadeSourceClip[]=["a","b","c"].map((id,i)=>({
    clip_id:id,asset_id:"asset-"+i,sha256:(i+1).toString(16).padStart(2,"0").repeat(32),
    bytes:1000+i,
    source_in:{ticks:"0",time_base:"1/1000000"},
    source_out:{ticks:"2000000",time_base:"1/1000000"},
  }));
  return {project:{project_id:"project-1",fps:30,clips:sources.map(s=>({id:s.clip_id,frames:60}))},project_revision:12,source_clips:sources};
};
const fades=[{from_clip_id:"a",to_clip_id:"b",frames:12},{from_clip_id:"b",to_clip_id:"c",frames:9}];

describe("experimental crossfade revision/source-bound snapshot",()=>{
  it("round-trips an exact saved revision and source binding",()=>{
    const context=makeContext();
    const json=encodeCrossfadeDraftSnapshot(context,fades);
    expect(decodeCrossfadeDraftSnapshot(json,context)).toEqual(fades);
    const parsed=JSON.parse(json);
    expect(parsed.version).toBe(1);
    expect(parsed.project_revision).toBe(12);
    expect(parsed.source_identity.clips.map((clip:CrossfadeSourceClip)=>clip.clip_id)).toEqual(["a","b","c"]);
  });
  it("treats an absent snapshot as empty but refuses malformed saved records",()=>{
    const context=makeContext();
    expect(decodeCrossfadeDraftSnapshot(null,context)).toEqual([]);
    expect(()=>decodeCrossfadeDraftSnapshot("",context)).toThrow(/size/);
    expect(()=>decodeCrossfadeDraftSnapshot("{",context)).toThrow(/JSON/);
    expect(()=>decodeCrossfadeDraftSnapshot(" ".repeat(131073),context)).toThrow(/size/);
  });
  it("refuses a revised project, unchanged-length source replacement and exact trim changes",()=>{
    const context=makeContext(),json=encodeCrossfadeDraftSnapshot(context,fades);
    expect(()=>decodeCrossfadeDraftSnapshot(json,{...context,project_revision:13})).toThrow(/revision/);
    const replacement=makeContext();replacement.source_clips[0].sha256="ff".repeat(32);
    expect(()=>decodeCrossfadeDraftSnapshot(json,replacement)).toThrow(/no longer matches/);
    const trim=makeContext();trim.source_clips[1].source_out.ticks="1999999";
    expect(()=>decodeCrossfadeDraftSnapshot(json,trim)).toThrow(/no longer matches/);
  });
  it("refuses reordered or missing clip identities, even with matching frame budgets",()=>{
    const context=makeContext(),json=encodeCrossfadeDraftSnapshot(context,fades);
    const reordered=makeContext();
    reordered.project.clips.reverse();reordered.source_clips=[...reordered.source_clips].reverse();
    expect(()=>decodeCrossfadeDraftSnapshot(json,reordered)).toThrow(/timeline|matches/);
    const inconsistent=makeContext();inconsistent.project.clips[1].id="different";
    expect(()=>encodeCrossfadeDraftSnapshot(inconsistent,fades)).toThrow(/disagree/);
    expect(()=>decodeCrossfadeDraftSnapshot(null,inconsistent)).toThrow(/disagree/);
  });
  it("rejects altered envelope shape, future version and corrupt inner record",()=>{
    const context=makeContext();
    const parsed=JSON.parse(encodeCrossfadeDraftSnapshot(context,fades));
    expect(()=>decodeCrossfadeDraftSnapshot(JSON.stringify({...parsed,version:2}),context)).toThrow(/revision/);
    expect(()=>decodeCrossfadeDraftSnapshot(JSON.stringify({...parsed,unexpected:true}),context)).toThrow(/revision/);
    expect(()=>decodeCrossfadeDraftSnapshot(JSON.stringify({...parsed,draft:"{}"}),context)).toThrow(/draft record/);
    expect(()=>decodeCrossfadeDraftSnapshot(JSON.stringify({...parsed,source_identity:{version:2,clips:[]}}),context)).toThrow(/source identity/);
  });
  it("does not modify caller-provided fade arrays or original source metadata",()=>{
    const context=makeContext(),original=structuredClone(fades),sources=structuredClone(context.source_clips);
    const encoded=encodeCrossfadeDraftSnapshot(context,fades);
    const decoded=decodeCrossfadeDraftSnapshot(encoded,context);
    decoded[0].frames=3;
    expect(fades).toEqual(original);
    expect(context.source_clips).toEqual(sources);
  });
  it("rejects invalid revision and invalid frame identity before serialization",()=>{
    const context=makeContext();
    expect(()=>encodeCrossfadeDraftSnapshot({...context,project_revision:-1},fades)).toThrow(/revision/);
    expect(()=>encodeCrossfadeDraftSnapshot({...context,project_revision:0.5},fades)).toThrow(/revision/);
    expect(()=>encodeCrossfadeDraftSnapshot({...context,project_revision:Number.MAX_SAFE_INTEGER+1},fades)).toThrow(/revision/);
  });
});
