/** First non-overlapping video transition. Its two half-fades preserve exact project duration. */
export type CutTransition={from_clip_id:string;to_clip_id:string;kind:"dip_to_black";duration_seconds:number};
export type CutClip={id:string;duration:number};

/** Validate adjacent boundaries and prevent overlapping fade envelopes within a short clip. */
export function validateCutTransitions(clips:CutClip[],transitions:CutTransition[],fps:number):string{
  if(![30,60].includes(fps))return "Unsupported project frame rate.";
  if(transitions.length>Math.max(0,clips.length-1))return "Too many video transitions.";
  const seen=new Set<string>();
  for(const transition of transitions){
    if(transition.kind!=="dip_to_black"||!Number.isFinite(transition.duration_seconds)||transition.duration_seconds<0.08||transition.duration_seconds>1.5)return "Choose a video dip duration between 0.08 and 1.5 seconds.";
    const index=clips.findIndex(clip=>clip.id===transition.from_clip_id);
    if(index<0||index+1>=clips.length||clips[index+1].id!==transition.to_clip_id||seen.has(transition.from_clip_id))return "Video transitions must join two adjacent clips exactly once.";
    seen.add(transition.from_clip_id);
  }
  for(let i=0;i<clips.length;i++){
    const entering=transitions.find(t=>t.to_clip_id===clips[i].id)?.duration_seconds||0;
    const exiting=transitions.find(t=>t.from_clip_id===clips[i].id)?.duration_seconds||0;
    if(!Number.isFinite(clips[i].duration)||clips[i].duration<(entering+exiting)/2+2/fps)return "A clip is too short for its selected video transitions.";
  }
  return "";
}

/** Remove transitions whose exact pair no longer shares a cut after a reorder/deletion. */
export function retainAdjacentTransitions(clips:CutClip[],transitions:CutTransition[]):CutTransition[]{
  return transitions.filter(t=>clips.some((clip,index)=>clip.id===t.from_clip_id&&clips[index+1]?.id===t.to_clip_id));
}
