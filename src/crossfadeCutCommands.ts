/** Phase J transition editing primitives, deliberately independent of persisted schemas.
 *
 * These functions validate prospective timeline edits against the existing
 * integer-frame clock before the editor can commit them. They do NOT enable
 * production crossfade preview or export.
 */
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";

export type CrossfadeEditResult={boundaries:CrossfadeBoundary[];changed:boolean};

/** The largest whole-frame overlap that leaves two untouched frames on each
 * adjacent clip, taking other incoming and outgoing overlaps into account.
 * A zero return value means this cut cannot accept a valid transition.
 */
export function maxCrossfadeFrames(
  clips:readonly CrossfadeClipClock[],current:readonly CrossfadeBoundary[],
  fromClipId:string,fps:30|60,
):number{
  if(fps!==30&&fps!==60)throw Error("Unsupported crossfade frame rate.");
  compileCrossfadeClock([...clips],[...current]);
  const index=clips.findIndex(clip=>clip.id===fromClipId);
  if(index<0||index===clips.length-1)return 0;
  const first=clips[index],second=clips[index+1];
  const previous=current.find(edge=>edge.to_clip_id===first.id)?.frames??0;
  const following=current.find(edge=>edge.from_clip_id===second.id)?.frames??0;
  const cap=Math.min(Math.floor(1.5*fps),first.frames-previous-2,second.frames-following-2);
  return cap>=Math.ceil(.08*fps)?cap:0;
}

/** Apply one user intention without mutating existing project state.
 * Invalid edits throw so callers can preserve the current undo/autosave state.
 * null removes the selected transition. Repeated identical edits are no-ops.
 */
export function setCrossfadeAtCut(
  clips:readonly CrossfadeClipClock[],current:readonly CrossfadeBoundary[],
  fromClipId:string,frames:number|null,fps:30|60,
):CrossfadeEditResult{
  if(fps!==30&&fps!==60)throw Error("Unsupported crossfade frame rate.");
  compileCrossfadeClock([...clips],[...current]);
  const index=clips.findIndex(clip=>clip.id===fromClipId);
  if(index<0||index+1>=clips.length)throw Error("Crossfade cut no longer exists.");
  const toClipId=clips[index+1].id;
  const existing=current.find(edge=>edge.from_clip_id===fromClipId);
  if(frames!==null){
    if(!Number.isSafeInteger(frames)||frames<Math.ceil(.08*fps)||frames>Math.floor(1.5*fps))
      throw Error("Crossfade duration must be between 0.08 and 1.5 seconds in whole frames.");
    // Exclude the cut being edited when computing remaining room, otherwise
    // increasing/decreasing an existing transition would be incorrectly limited.
    const other=current.filter(edge=>edge.from_clip_id!==fromClipId);
    if(frames>maxCrossfadeFrames(clips,other,fromClipId,fps))
      throw Error("Crossfade would exhaust one of its adjacent clips.");
  }
  if((frames===null&&!existing)||(frames!==null&&existing?.frames===frames))
    return {boundaries:[...current],changed:false};
  const next=current.filter(edge=>edge.from_clip_id!==fromClipId);
  if(frames!==null)next.push({from_clip_id:fromClipId,to_clip_id:toClipId,frames});
  // Stable project-order serialization independent of which cut was edited last.
  const order=new Map(clips.map((clip,i)=>[clip.id,i]));
  next.sort((a,b)=>(order.get(a.from_clip_id)??0)-(order.get(b.from_clip_id)??0));
  compileCrossfadeClock([...clips],next);
  return {boundaries:next,changed:true};
}
