/** Experimental crossfade draft history.
 *
 * Phase J preparation only: these snapshots are held in memory and do not
 * extend schema v3 or write into the existing production save queue.
 */
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";
import {setCrossfadeAtCut} from "./crossfadeCutCommands";
import {editHistory,type History} from "./projectSession";

export type CrossfadeDraft={
  clips:CrossfadeClipClock[];
  boundaries:CrossfadeBoundary[];
  fps:30|60;
};
export type CrossfadeDraftAction=
  |{type:"set";fromClipId:string;frames:number|null}
  |{type:"undo"}
  |{type:"redo"};

function verifiedDraft(draft:CrossfadeDraft):CrossfadeDraft{
  if(draft.fps!==30&&draft.fps!==60)throw Error("Unsupported crossfade frame rate.");
  compileCrossfadeClock(draft.clips,draft.boundaries);
  // Never expose references to caller-owned mutable state in the draft history.
  return {
    fps:draft.fps,
    clips:draft.clips.map(clip=>({...clip})),
    boundaries:draft.boundaries.map(edge=>({...edge})),
  };
}

/** Begin a tentative edit session without altering the real project. */
export function startCrossfadeDraft(draft:CrossfadeDraft):History<CrossfadeDraft>{
  return {past:[],present:verifiedDraft(draft),future:[]};
}

/** One edit = one history entry; rejected edits and no-ops preserve history
 * identity, including the redo stack. Undo/redo use the shared editor's
 * bounded history semantics.
 */
export function editCrossfadeDraft(
  history:History<CrossfadeDraft>,
  action:CrossfadeDraftAction,
):History<CrossfadeDraft>{
  if(action.type==="undo"||action.type==="redo"){
    const result=editHistory(history,action.type);
    if(result!==history)verifiedDraft(result.present);
    return result;
  }
  verifiedDraft(history.present);
  const result=setCrossfadeAtCut(
    history.present.clips,history.present.boundaries,
    action.fromClipId,action.frames,history.present.fps,
  );
  if(!result.changed)return history;
  const next=verifiedDraft({...history.present,boundaries:result.boundaries});
  return editHistory(history,next);
}
