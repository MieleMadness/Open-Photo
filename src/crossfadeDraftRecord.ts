/** Isolated Phase J serialization contract for *experimental* crossfade drafts.
 *
 * Not a production project schema migration or persistence implementation.
 * A later reviewed adapter must decide where (and whether) this is stored.
 */
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";

export type CrossfadeDraftIdentity={
  project_id:string;
  fps:30|60;
  clips:CrossfadeClipClock[];
};
export type CrossfadeDraftRecord={
  version:1;
  project_id:string;
  fps:30|60;
  clips:CrossfadeClipClock[];
  boundaries:CrossfadeBoundary[];
};

const MAX_BYTES=65536;
const ID=/^[a-zA-Z0-9-]{1,120}$/;

function plainRecord(value:unknown):value is Record<string,unknown>{
  return typeof value==="object"&&value!==null&&!Array.isArray(value)&&Object.getPrototypeOf(value)===Object.prototype;
}
function keysAre(value:Record<string,unknown>,keys:readonly string[]):boolean{
  const actual=Object.keys(value);
  return actual.length===keys.length&&actual.every(key=>keys.includes(key));
}
function checkedIdentity(identity:CrossfadeDraftIdentity):CrossfadeDraftIdentity{
  if(!ID.test(identity.project_id)||![30,60].includes(identity.fps)||identity.clips.length>200)
    throw Error("Invalid crossfade draft project identity.");
  const total=identity.clips.reduce((sum,clip)=>sum+clip.frames,0);
  if(!Number.isSafeInteger(total)||total>7200*identity.fps)
    throw Error("Crossfade draft exceeds supported project bounds.");
  compileCrossfadeClock(identity.clips,[]);
  return identity;
}
function checkedBoundaries(identity:CrossfadeDraftIdentity,boundaries:CrossfadeBoundary[]):CrossfadeBoundary[]{
  if(boundaries.some(edge=>!Number.isSafeInteger(edge.frames)
    ||edge.frames<Math.ceil(.08*identity.fps)||edge.frames>Math.floor(1.5*identity.fps)))
    throw Error("Crossfade draft transition duration is outside supported frame bounds.");
  const clock=compileCrossfadeClock(identity.clips,boundaries);
  if(clock.total_frames>7200*identity.fps)throw Error("Crossfade timeline exceeds two hours.");
  return [...boundaries].map(edge=>({...edge}));
}
/** Canonical JSON is bounded, versioned, and bound to the saved clip-frame
 * identity. Caller input is never mutated.
 */
export function encodeCrossfadeDraft(
  identity:CrossfadeDraftIdentity,boundaries:CrossfadeBoundary[],
):string{
  checkedIdentity(identity);
  const result:CrossfadeDraftRecord={
    version:1,project_id:identity.project_id,fps:identity.fps,
    clips:identity.clips.map(clip=>({...clip})),
    boundaries:checkedBoundaries(identity,boundaries),
  };
  const json=JSON.stringify(result);
  if(json.length>MAX_BYTES)throw Error("Crossfade draft record is too large.");
  return json;
}
/** Strict decoder for a future opt-in draft store. Missing records can be
 * treated as an empty crossfade draft; malformed or stale records MUST fail
 * loudly rather than silently falling back and destroying saved intent.
 */
export function decodeCrossfadeDraft(
  json:string|null,expected:CrossfadeDraftIdentity,
):CrossfadeBoundary[]{
  checkedIdentity(expected);
  if(json===null)return [];
  if(typeof json!=="string"||json.length===0||json.length>MAX_BYTES)
    throw Error("Invalid crossfade draft record size.");
  let raw:unknown;
  try{raw=JSON.parse(json);}catch{throw Error("Crossfade draft record is not valid JSON.");}
  if(!plainRecord(raw)||!keysAre(raw,["version","project_id","fps","clips","boundaries"])
    ||raw.version!==1||raw.project_id!==expected.project_id||raw.fps!==expected.fps
    ||!Array.isArray(raw.clips)||!Array.isArray(raw.boundaries))
    throw Error("Unsupported or mismatched crossfade draft record.");
  if(raw.clips.length!==expected.clips.length||raw.boundaries.length>Math.max(0,expected.clips.length-1))
    throw Error("Crossfade draft no longer matches its timeline.");
  for(let i=0;i<raw.clips.length;i++){
    const clip:unknown=raw.clips[i],current=expected.clips[i];
    if(!plainRecord(clip)||!keysAre(clip,["id","frames"])||clip.id!==current.id||clip.frames!==current.frames)
      throw Error("Crossfade draft no longer matches its timeline.");
  }
  const boundaries:CrossfadeBoundary[]=[];
  for(const edge of raw.boundaries){
    if(!plainRecord(edge)||!keysAre(edge,["from_clip_id","to_clip_id","frames"])
      ||typeof edge.from_clip_id!=="string"||typeof edge.to_clip_id!=="string"
      ||!Number.isSafeInteger(edge.frames))
      throw Error("Invalid crossfade draft transition.");
    boundaries.push({
      from_clip_id:edge.from_clip_id,to_clip_id:edge.to_clip_id,frames:edge.frames as number,
    });
  }
  return checkedBoundaries(expected,boundaries);
}
