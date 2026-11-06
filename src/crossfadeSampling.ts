import {type CrossfadeClock} from "./crossfadeClock";

/** One source frame sampled at an integer position in its retained clip range. */
export type CrossfadeFrameSource={clip_id:string;source_frame:number;video_weight:number;audio_weight:number};
export type CrossfadeFrameSample={project_frame:number;sources:CrossfadeFrameSource[]};

/** Determine the source frame(s) for one project-frame slot.
 *
 * Linear video alpha weights create a continuous visual dissolve. Equal-power
 * audio weights are a *future renderer contract* only; actual multi-stream
 * FFmpeg audio mixing has not been implemented in this patch.
 *
 * Sample centers are used at a cut to avoid a duplicate endpoint frame.
 */
export function sampleCrossfadeFrame(clock:CrossfadeClock,frame:number):CrossfadeFrameSample{
  if(!Number.isSafeInteger(frame)||frame<0||frame>=clock.total_frames)
    throw Error("Frame is outside the crossfade timeline.");

  const active=clock.entries.filter(clip=>frame>=clip.start_frame&&frame<clip.end_frame);
  if(active.length<1||active.length>2)
    throw Error("A crossfade frame must have one or two source clips.");

  if(active.length===1){
    const clip=active[0];
    return {project_frame:frame,sources:[{clip_id:clip.id,source_frame:frame-clip.start_frame,video_weight:1,audio_weight:1}]};
  }
  const [outgoing,incoming]=active;
  const overlap=clock.overlaps.find(item=>
    item.from_clip_id===outgoing.id&&item.to_clip_id===incoming.id&&
    frame>=item.start_frame&&frame<item.end_frame
  );
  if(!overlap)throw Error("Overlapping clips need a declared crossfade boundary.");

  const progress=(frame-overlap.start_frame+0.5)/overlap.frames;
  const theta=progress*Math.PI/2;
  return {project_frame:frame,sources:[
    {clip_id:outgoing.id,source_frame:frame-outgoing.start_frame,video_weight:1-progress,audio_weight:Math.cos(theta)},
    {clip_id:incoming.id,source_frame:frame-incoming.start_frame,video_weight:progress,audio_weight:Math.sin(theta)},
  ]};
}
