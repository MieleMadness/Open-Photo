/** Future true two-source crossfades: frame-clock layout only.
 *
 * This module does not change the existing hard-cut/dip renderer or project schema.
 * It defines explicit overlapping timing so native audio/video composition can use
 * one clock when the crossfade feature is implemented and verified.
 */
export type CrossfadeClipClock={id:string;frames:number};
export type CrossfadeBoundary={from_clip_id:string;to_clip_id:string;frames:number};
export type CrossfadeClockEntry={id:string;start_frame:number;end_frame:number;frames:number};
export type CrossfadeClockOverlap={from_clip_id:string;to_clip_id:string;start_frame:number;end_frame:number;frames:number};
export type CrossfadeClock={entries:CrossfadeClockEntry[];overlaps:CrossfadeClockOverlap[];total_frames:number};

/** Reject ambiguous identity, invalid cuts or fades that exhaust a clip.
 * Two clean frames remain in every clip outside its incoming/outgoing overlaps.
 */
export function compileCrossfadeClock(clips:CrossfadeClipClock[],edges:CrossfadeBoundary[]):CrossfadeClock{
  const seen=new Set<string>();
  for(const clip of clips){
    if(!/^[a-zA-Z0-9-]{1,120}$/.test(clip.id)||seen.has(clip.id)||!Number.isSafeInteger(clip.frames)||clip.frames<2)
      throw Error("Crossfade clips require unique IDs and at least two frames.");
    seen.add(clip.id);
  }
  if(edges.length>Math.max(0,clips.length-1))throw Error("Too many crossfade boundaries.");
  const boundaries=new Map<string,CrossfadeBoundary>();
  for(const edge of edges){
    const i=clips.findIndex(clip=>clip.id===edge.from_clip_id);
    if(i<0||i+1>=clips.length||clips[i+1].id!==edge.to_clip_id||boundaries.has(edge.from_clip_id))
      throw Error("A crossfade must join adjacent clips exactly once.");
    if(!Number.isSafeInteger(edge.frames)||edge.frames<1)
      throw Error("Crossfade duration must contain whole project frames.");
    boundaries.set(edge.from_clip_id,edge);
  }
  for(let i=0;i<clips.length;i++){
    const incoming=i>0?boundaries.get(clips[i-1].id)?.frames||0:0;
    const outgoing=boundaries.get(clips[i].id)?.frames||0;
    if(incoming+outgoing+2>clips[i].frames)
      throw Error("Crossfade duration exceeds the available clip frames.");
  }
  const entries:CrossfadeClockEntry[]=[],overlaps:CrossfadeClockOverlap[]=[];
  let previousEnd=0;
  for(let i=0;i<clips.length;i++){
    const clip=clips[i];
    const overlap=i>0?boundaries.get(clips[i-1].id)?.frames||0:0;
    const start=previousEnd-overlap,end=start+clip.frames;
    if(!Number.isSafeInteger(start)||!Number.isSafeInteger(end))
      throw Error("Crossfade timeline exceeds the safe frame range.");
    entries.push({id:clip.id,start_frame:start,end_frame:end,frames:clip.frames});
    if(overlap>0)overlaps.push({from_clip_id:clips[i-1].id,to_clip_id:clip.id,start_frame:start,end_frame:previousEnd,frames:overlap});
    previousEnd=end;
  }
  return {entries,overlaps,total_frames:previousEnd};
}

/** Clock positions remain integers until the UI must display seconds. */
export function crossfadeFrameSeconds(frame:number,fps:30|60):number{
  if(!Number.isSafeInteger(frame)||frame<0)throw Error("Invalid project frame.");
  return frame/fps;
}
