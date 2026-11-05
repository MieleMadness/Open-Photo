/** Experimental Phase J draft editing workflow (in memory only).
 *
 * This adapter binds the existing cut-history primitives to a captured,
 * validated source+revision snapshot. It performs no native save or render
 * and deliberately does not modify the production project schema.
 */
import {type CrossfadeBoundary} from "./crossfadeClock";
import {startCrossfadeDraft,editCrossfadeDraft,type CrossfadeDraft,type CrossfadeDraftAction} from "./crossfadeDraftHistory";
import {decodeCrossfadeDraftSnapshot,encodeCrossfadeDraftSnapshot,type CrossfadeDraftSnapshotContext} from "./crossfadeDraftSnapshot";
import {type History} from "./projectSession";

export type CrossfadeDraftWorkflow={
  readonly baseline:string;
  readonly original_json:string|null;
  readonly original_boundaries:CrossfadeBoundary[];
  readonly history:History<CrossfadeDraft>;
};

function snapshotContextToken(context:CrossfadeDraftSnapshotContext):string{
  // Use the same strict validators as the actual wire format; empty boundaries
  // mean this token records identity alone, not the current editing decision.
  return encodeCrossfadeDraftSnapshot(context,[]);
}

/** Open only a validated saved record. Stale/malformed data causes a visible
 * error instead of an implicit reset to an empty crossfade draft.
 */
export function openCrossfadeDraftWorkflow(
  context:CrossfadeDraftSnapshotContext,
  savedJson:string|null,
):CrossfadeDraftWorkflow{
  const baseline=snapshotContextToken(context);
  const boundaries=decodeCrossfadeDraftSnapshot(savedJson,context);
  return {
    baseline,original_json:savedJson,
    original_boundaries:boundaries.map(edge=>({...edge})),
    history:startCrossfadeDraft({
      fps:context.project.fps,
      clips:context.project.clips.map(clip=>({...clip})),
      boundaries,
    }),
  };
}

/** One accepted transition change is one undo/redo history entry. */
export function updateCrossfadeDraftWorkflow(
  workflow:CrossfadeDraftWorkflow,
  action:CrossfadeDraftAction,
):CrossfadeDraftWorkflow{
  const next=editCrossfadeDraft(workflow.history,action);
  return next===workflow.history?workflow:{...workflow,history:next};
}

/** Verify the project has not changed since the draft was opened.
 * If edits were undone back to their original state, retain the original
 * serialized bytes (or null) and avoid a needless storage write.
 *
 * The returned JSON is only a candidate for a future transactional save API.
 */
export function finalizeCrossfadeDraftWorkflow(
  workflow:CrossfadeDraftWorkflow,
  current:CrossfadeDraftSnapshotContext,
):{changed:boolean;json:string|null}{
  if(snapshotContextToken(current)!==workflow.baseline)
    throw Error("Crossfade draft project changed; reload before saving.");
  const edges=workflow.history.present.boundaries;
  const same=JSON.stringify(edges)===JSON.stringify(workflow.original_boundaries);
  if(same)return {changed:false,json:workflow.original_json};
  const json=encodeCrossfadeDraftSnapshot(current,edges);
  // Check the serialized candidate through the same strict load path.
  decodeCrossfadeDraftSnapshot(json,current);
  return {changed:true,json};
}
