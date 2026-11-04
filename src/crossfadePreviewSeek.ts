/** Experimental Phase J crossfade preview seeking.
 *
 * Pure frame-clock adapter only. This does not invoke native media, mutate
 * project state, or enable the production hard-cut/dip preview/export paths.
 */
import {type CrossfadeClock} from "./crossfadeClock";
import {sampleCrossfadeFrame,type CrossfadeFrameSource} from "./crossfadeSampling";

export type CrossfadePreviewSource=CrossfadeFrameSource&{
  /** Offset from this clip's trimmed source_in mark, not an absolute file PTS. */
  source_offset_seconds:number;
};
export type CrossfadePreviewPosition={
  project_frame:number;
  project_time_seconds:number;
  in_overlap:boolean;
  sources:CrossfadePreviewSource[];
};

/** Quantize the editor's approximate playhead to the discrete output frame.
 * Project end is an exclusive timestamp; callers should stop playback there,
 * rather than silently rendering a duplicate last frame.
 */
export function crossfadePreviewAtTime(
  clock:CrossfadeClock,
  fps:30|60,
  seconds:number,
):CrossfadePreviewPosition{
  if(fps!==30&&fps!==60)throw Error("Unsupported crossfade preview frame rate.");
  if(!Number.isFinite(seconds)||seconds<0||!Number.isSafeInteger(clock.total_frames)||clock.total_frames<1)
    throw Error("Invalid crossfade preview timestamp.");
  const totalSeconds=clock.total_frames/fps;
  if(seconds>=totalSeconds)throw Error("Crossfade preview timestamp is outside project duration.");
  // A tiny tolerance avoids a prior frame from floating-point representation
  // at exact integer-frame cut points; never cross the exclusive project end.
  const frame=Math.floor(seconds*fps+1e-9);
  if(!Number.isSafeInteger(frame)||frame>=clock.total_frames)
    throw Error("Crossfade preview timestamp is outside project duration.");
  const sample=sampleCrossfadeFrame(clock,frame);
  return {
    project_frame:frame,
    project_time_seconds:frame/fps,
    in_overlap:sample.sources.length===2,
    sources:sample.sources.map(source=>({
      ...source,source_offset_seconds:source.source_frame/fps,
    })),
  };
}
