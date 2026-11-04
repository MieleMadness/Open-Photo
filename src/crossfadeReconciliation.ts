/** Reconcile experimental crossfade cut definitions after a timeline mutation.
 *
 * This is a pure, opt-in Phase J helper; the production schema/render path
 * remains unchanged. Discarded edges are reported so UI can disclose
 * transition loss rather than silently modifying saved output.
 */
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClipClock} from "./crossfadeClock";

export type CrossfadeReconciliation={
  boundaries:CrossfadeBoundary[];
  removed:CrossfadeBoundary[];
};

/** Keep only still-adjacent, frame-safe overlaps. Prefer the earliest
 * timeline cut if two old transitions compete for a shortened clip budget.
 * Malformed/duplicate input edges are dropped, never coerced into output.
 */
export function reconcileCrossfadeCuts(
  clips:readonly CrossfadeClipClock[],
  previous:readonly CrossfadeBoundary[],
):CrossfadeReconciliation{
  compileCrossfadeClock([...clips],[]);
  const positions=new Map(clips.map((clip,index)=>[clip.id,index]));
  const candidates=previous.map((edge,index)=>({edge,index})).sort((a,b)=>{
    const left=positions.get(a.edge.from_clip_id)??Number.MAX_SAFE_INTEGER;
    const right=positions.get(b.edge.from_clip_id)??Number.MAX_SAFE_INTEGER;
    return left-right||a.index-b.index;
  });
  const accepted:CrossfadeBoundary[]=[];
  const acceptedIndexes=new Set<number>();
  for(const {edge,index} of candidates){
    const position=positions.get(edge.from_clip_id);
    if(position===undefined||position>=clips.length-1||clips[position+1].id!==edge.to_clip_id)
      continue;
    try{
      compileCrossfadeClock([...clips],[...accepted,edge]);
      accepted.push({...edge});
      acceptedIndexes.add(index);
    }catch{
      // A formerly valid edge may exceed a shortened clip or conflict with
      // another transition. Never silently clamp it to a different duration.
    }
  }
  return {
    boundaries:accepted,
    removed:previous.filter((_,index)=>!acceptedIndexes.has(index)).map(edge=>({...edge})),
  };
}
