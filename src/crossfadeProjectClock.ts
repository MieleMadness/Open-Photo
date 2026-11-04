/** Convert saved source marks to the same cumulative project-frame grid as
 * `video_sequence_render::frame_allocation` in Rust.
 *
 * Experimental crossfade-only adapter: it does not change persisted v3 project
 * documents, the editor playhead or the hard-cut/dip preview/export path.
 */
import {compileCrossfadeClock,type CrossfadeBoundary,type CrossfadeClock} from "./crossfadeClock";
import {markSeconds,type VideoTime} from "./videoTime";

export type CrossfadeMarkedClip={id:string;source_in:VideoTime;source_out:VideoTime};
export type CrossfadeProjectRate=30|60;

/** Quantize an intended transition duration into complete project frames.
 * The UI must show the quantized value, not silently store fractional frames.
 */
export function crossfadeDurationFrames(seconds:number,fps:CrossfadeProjectRate):number{
  if(![30,60].includes(fps)||!Number.isFinite(seconds)||seconds<0.08||seconds>1.5)
    throw Error("Choose a crossfade duration between 0.08 and 1.5 seconds.");
  const frames=Math.round(seconds*fps);
  if(!Number.isSafeInteger(frames)||frames<1)
    throw Error("Crossfade duration cannot be represented in project frames.");
  return frames;
}

/** Allocation deliberately rounds cumulative duration once per cut, mirroring Rust.
 * Rounding every trimmed clip independently causes project-clock drift.
 */
export function compileMarkedCrossfadeClock(
  clips:CrossfadeMarkedClip[],
  fps:CrossfadeProjectRate,
  boundaries:CrossfadeBoundary[],
):CrossfadeClock{
  if(![30,60].includes(fps))throw Error("Unsupported crossfade project frame rate.");
  let elapsed=0,previousFrame=0;
  const frames=clips.map(clip=>{
    const first=markSeconds(clip.source_in),last=markSeconds(clip.source_out);
    const duration=last-first;
    if(!Number.isFinite(first)||!Number.isFinite(last)||!Number.isFinite(duration)||first<0||duration<=0)
      throw Error("Invalid crossfade source marks.");
    elapsed+=duration;
    if(!Number.isFinite(elapsed)||elapsed>7200)
      throw Error("Project duration exceeds two hours.");
    const endFrame=Math.round(elapsed*fps);
    const count=endFrame-previousFrame;
    if(!Number.isSafeInteger(endFrame)||count<2)
      throw Error("A crossfade candidate must contain at least two allocated project frames.");
    previousFrame=endFrame;
    return {id:clip.id,frames:count};
  });
  return compileCrossfadeClock(frames,boundaries);
}
