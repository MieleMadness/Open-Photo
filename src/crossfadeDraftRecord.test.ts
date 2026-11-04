import {describe,expect,it} from "vitest";
import {decodeCrossfadeDraft,encodeCrossfadeDraft,type CrossfadeDraftIdentity} from "./crossfadeDraftRecord";

const initial=():CrossfadeDraftIdentity=>({
  project_id:"saved-project-1",fps:30,
  clips:[{id:"first",frames:90},{id:"middle",frames:60},{id:"last",frames:90}],
});
const edges=[
  {from_clip_id:"first",to_clip_id:"middle",frames:15},
  {from_clip_id:"middle",to_clip_id:"last",frames:12},
];

describe("Phase J versioned experimental draft record",()=>{
  it("round-trips validated cut identity with deterministic JSON",()=>{
    const identity=initial();
    const json=encodeCrossfadeDraft(identity,edges);
    expect(JSON.parse(json)).toEqual({version:1,...identity,boundaries:edges});
    expect(decodeCrossfadeDraft(json,identity)).toEqual(edges);
    expect(encodeCrossfadeDraft(identity,decodeCrossfadeDraft(json,identity))).toBe(json);
  });
  it("treats absent drafts as zero crossfades without masking corrupt saved data",()=>{
    expect(decodeCrossfadeDraft(null,initial())).toEqual([]);
    expect(()=>decodeCrossfadeDraft("",initial())).toThrow(/size/);
    expect(()=>decodeCrossfadeDraft("{",initial())).toThrow(/JSON/);
    expect(()=>decodeCrossfadeDraft(" ".repeat(65537),initial())).toThrow(/size/);
  });
  it("refuses stale clip length, order, identity, fps and project ID",()=>{
    const identity=initial(),json=encodeCrossfadeDraft(identity,edges);
    const changedLength=initial();changedLength.clips[1].frames++;
    expect(()=>decodeCrossfadeDraft(json,changedLength)).toThrow(/timeline/);
    const reordered=initial();reordered.clips.reverse();
    expect(()=>decodeCrossfadeDraft(json,reordered)).toThrow(/timeline/);
    expect(()=>decodeCrossfadeDraft(json,{...identity,project_id:"other"})).toThrow(/mismatched/);
    expect(()=>decodeCrossfadeDraft(json,{...identity,fps:60})).toThrow(/mismatched/);
  });
  it("rejects future versions, unknown root fields and malformed edge shapes",()=>{
    const identity=initial(),raw=JSON.parse(encodeCrossfadeDraft(identity,edges));
    expect(()=>decodeCrossfadeDraft(JSON.stringify({...raw,version:2}),identity)).toThrow(/mismatched/);
    expect(()=>decodeCrossfadeDraft(JSON.stringify({...raw,unexpected:true}),identity)).toThrow(/mismatched/);
    expect(()=>decodeCrossfadeDraft(JSON.stringify({...raw,clips:[{...raw.clips[0],other:1},...raw.clips.slice(1)]}),identity)).toThrow(/timeline/);
    expect(()=>decodeCrossfadeDraft(JSON.stringify({...raw,boundaries:[{...edges[0],other:1}]}),identity)).toThrow(/transition/);
    expect(()=>decodeCrossfadeDraft(JSON.stringify({...raw,boundaries:[{...edges[0],frames:0.5}]}),identity)).toThrow(/transition/);
  });
  it("fails closed for duplicate, nonadjacent, exhausted or over-limit transitions",()=>{
    const identity=initial();
    expect(()=>encodeCrossfadeDraft(identity,[edges[0],edges[0]])).toThrow(/exactly once/);
    expect(()=>encodeCrossfadeDraft(identity,[{...edges[0],to_clip_id:"last"}])).toThrow(/adjacent/);
    expect(()=>encodeCrossfadeDraft(identity,[{...edges[0],frames:46}])).toThrow(/duration/);
    expect(()=>encodeCrossfadeDraft(identity,[{...edges[0],frames:2}])).toThrow(/duration/);
    const short=initial();short.clips[1].frames=15;
    expect(()=>encodeCrossfadeDraft(short,edges)).toThrow(/available clip frames/);
  });
  it("rejects projects beyond the supported clip or duration budgets",()=>{
    const tooMany=initial();tooMany.clips=Array.from({length:201},(_,index)=>({id:"clip-"+index,frames:2}));
    expect(()=>encodeCrossfadeDraft(tooMany,[])).toThrow(/identity/);
    const tooLong=initial();tooLong.clips[0].frames=7200*30;
    expect(()=>encodeCrossfadeDraft(tooLong,[])).toThrow(/bounds/);
    expect(()=>encodeCrossfadeDraft({...initial(),project_id:"bad id"},[])).toThrow(/identity/);
  });
  it("returns detached copies and leaves caller-owned input untouched",()=>{
    const identity=initial(),original=structuredClone(edges);
    const json=encodeCrossfadeDraft(identity,edges);
    const decoded=decodeCrossfadeDraft(json,identity);
    decoded[0].frames=30;
    expect(edges).toEqual(original);
    expect(decodeCrossfadeDraft(json,identity)).toEqual(original);
  });
});
