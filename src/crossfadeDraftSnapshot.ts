/** Opt-in, test-only Phase J crossfade draft snapshot envelope.
 *
 * Combines versioned frame-clock boundaries, exact source identity and a
 * captured project revision. No filesystem writes or production schema change.
 */
import {type CrossfadeBoundary} from "./crossfadeClock";
import {decodeCrossfadeDraft,encodeCrossfadeDraft,type CrossfadeDraftIdentity} from "./crossfadeDraftRecord";
import {captureCrossfadeSourceIdentity,requireCrossfadeSourceIdentity,type CrossfadeSourceClip,type CrossfadeSourceIdentity} from "./crossfadeSourceIdentity";

export type CrossfadeDraftSnapshot={
  version:1;
  project_revision:number;
  source_identity:CrossfadeSourceIdentity;
  draft:string;
};
export type CrossfadeDraftSnapshotContext={
  project:CrossfadeDraftIdentity;
  project_revision:number;
  source_clips:readonly CrossfadeSourceClip[];
};
const MAX_SNAPSHOT_CHARS=131072;
function checkContext(context:CrossfadeDraftSnapshotContext):void{
  if(!Number.isSafeInteger(context.project_revision)||context.project_revision<0)
    throw Error("Invalid crossfade snapshot project revision.");
  if(context.project.clips.length!==context.source_clips.length
    ||context.project.clips.some((clip,i)=>clip.id!==context.source_clips[i].clip_id))
    throw Error("Crossfade snapshot source and frame clocks disagree.");
}
function exactKeys(raw:Record<string,unknown>,keys:readonly string[]):boolean{
  const actual=Object.keys(raw);
  return actual.length===keys.length&&actual.every(key=>keys.includes(key));
}
/** Build a detached snapshot suitable only for a future reviewed opt-in store.
 * This creates no file and does not grant an editor the ability to export.
 */
export function encodeCrossfadeDraftSnapshot(
  context:CrossfadeDraftSnapshotContext,
  boundaries:CrossfadeBoundary[],
):string{
  checkContext(context);
  const payload:CrossfadeDraftSnapshot={
    version:1,
    project_revision:context.project_revision,
    source_identity:captureCrossfadeSourceIdentity(context.source_clips),
    draft:encodeCrossfadeDraft(context.project,boundaries),
  };
  const json=JSON.stringify(payload);
  if(json.length>MAX_SNAPSHOT_CHARS)throw Error("Crossfade snapshot is too large.");
  return json;
}
/** Fail closed for any incompatible, stale or corrupted existing draft.
 * Absence is distinct from invalid data: null means no draft, but malformed
 * data must be surfaced to the user, never silently reset on load.
 */
export function decodeCrossfadeDraftSnapshot(
  json:string|null,
  context:CrossfadeDraftSnapshotContext,
):CrossfadeBoundary[]{
  checkContext(context);
  // Validate even when no saved snapshot is present.
  captureCrossfadeSourceIdentity(context.source_clips);
  if(json===null)return [];
  if(typeof json!=="string"||json.length===0||json.length>MAX_SNAPSHOT_CHARS)
    throw Error("Invalid crossfade snapshot size.");
  let raw:unknown;
  try{raw=JSON.parse(json);}catch{throw Error("Crossfade snapshot is not valid JSON.");}
  if(typeof raw!=="object"||raw===null||Array.isArray(raw))
    throw Error("Invalid crossfade snapshot envelope.");
  const record=raw as Record<string,unknown>;
  if(!exactKeys(record,["version","project_revision","source_identity","draft"])
    ||record.version!==1||!Number.isSafeInteger(record.project_revision)
    ||record.project_revision!==context.project_revision||typeof record.draft!=="string")
    throw Error("Unsupported or stale crossfade snapshot revision.");
  const identity=record.source_identity as CrossfadeSourceIdentity;
  requireCrossfadeSourceIdentity(identity,context.source_clips);
  return decodeCrossfadeDraft(record.draft,context.project);
}
