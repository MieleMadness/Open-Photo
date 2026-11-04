/** Experimental source-aware identity for a future crossfade draft.
 *
 * This binds cut layouts to saved source fingerprints and exact rational
 * trim marks, not just clip frame counts. It does not read files, verify
 * SHA-256 bytes, change schema v3 or authorize native export.
 */
import {type VideoTime} from "./videoTime";

export type CrossfadeSourceClip={
  clip_id:string;
  asset_id:string;
  sha256:string;
  bytes:number;
  source_in:VideoTime;
  source_out:VideoTime;
};
export type CrossfadeSourceIdentity={
  version:1;
  clips:CrossfadeSourceClip[];
};
const ID=/^[A-Za-z0-9-]{1,120}$/;
const HASH=/^[a-fA-F0-9]{64}$/;
const TICKS=/^(?:0|[1-9][0-9]{0,19})$/;
const BASE=/^([1-9][0-9]*)\/([1-9][0-9]*)$/;

/** Compare rational source times without reducing them to floating point.
 * This prevents microsecond-level trim changes from being rounded away.
 */
function positiveDuration(start:VideoTime,end:VideoTime):boolean{
  const a=BASE.exec(start.time_base),b=BASE.exec(end.time_base);
  if(!a||!b||!TICKS.test(start.ticks)||!TICKS.test(end.ticks))return false;
  const left=BigInt(start.ticks)*BigInt(a[1])*BigInt(b[2]);
  const right=BigInt(end.ticks)*BigInt(b[1])*BigInt(a[2]);
  return right>left;
}
function validateClip(clip:CrossfadeSourceClip):void{
  if(!clip||typeof clip.clip_id!=="string"||typeof clip.asset_id!=="string"||typeof clip.sha256!=="string"\n    ||!clip.source_in||!clip.source_out||typeof clip.source_in.ticks!=="string"\n    ||typeof clip.source_out.ticks!=="string"||typeof clip.source_in.time_base!=="string"\n    ||typeof clip.source_out.time_base!=="string"||!ID.test(clip.clip_id)||!ID.test(clip.asset_id)||!HASH.test(clip.sha256)
    ||!Number.isSafeInteger(clip.bytes)||clip.bytes<1
    ||!positiveDuration(clip.source_in,clip.source_out))
    throw Error("Invalid crossfade source binding.");
}
export function captureCrossfadeSourceIdentity(clips:readonly CrossfadeSourceClip[]):CrossfadeSourceIdentity{
  if(clips.length>200)throw Error("Crossfade source count exceeds project limit.");
  const seen=new Set<string>();
  const copy=clips.map(clip=>{
    validateClip(clip);
    if(seen.has(clip.clip_id))throw Error("Duplicate crossfade clip identity.");
    seen.add(clip.clip_id);
    return {...clip,sha256:clip.sha256.toLowerCase(),
      source_in:{...clip.source_in},source_out:{...clip.source_out}};
  });
  return {version:1,clips:copy};
}
/** Strict check before reopening an experimental draft.
 * Different source, hash, size, clip order or exact trim mark fails closed.
 * Asset reuse by distinct clip IDs is valid.
 */
export function requireCrossfadeSourceIdentity(
  saved:CrossfadeSourceIdentity,
  current:readonly CrossfadeSourceClip[],
):void{
  if(saved?.version!==1||!Array.isArray(saved.clips))
    throw Error("Unsupported crossfade source identity.");
  const actual=captureCrossfadeSourceIdentity(current);
  const expected=captureCrossfadeSourceIdentity(saved.clips);
  if(JSON.stringify(expected)!==JSON.stringify(actual))
    throw Error("Crossfade source binding no longer matches the saved project.");
}
