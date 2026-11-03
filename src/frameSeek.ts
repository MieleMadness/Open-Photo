/** Approximate decoded-video frame stepping using the reported average frame rate.
 * Fractional and variable frame rate sources can have timestamp gaps: this is
 * an inspection aid, not an authoritative edit mark or exact frame decoder.
 */
export function playbackFrameRate(rate:string|null|undefined):number|null {
  if(!rate)return null;
  const match=/^(\d+)(?:\/(\d+))?$/.exec(rate);
  if(!match)return null;
  const numerator=Number(match[1]),denominator=Number(match[2]||"1");
  if(!Number.isSafeInteger(numerator)||!Number.isSafeInteger(denominator)||numerator<=0||denominator<=0)return null;
  const fps=numerator/denominator;
  return Number.isFinite(fps)&&fps>=1&&fps<=240?fps:null;
}

/** Advance from the current playhead to the adjacent nominal frame clock tick. */
export function adjacentPlaybackFrame(position:number,rate:string|null|undefined,direction:-1|1,duration:number):number|null {
  const fps=playbackFrameRate(rate);
  if(fps===null||!Number.isFinite(position)||!Number.isFinite(duration)||duration<=0)return null;
  const clamped=Math.max(0,Math.min(duration,position));
  const clock=clamped*fps;
  // For imprecise HTML media timestamps, snap to the nearest frame first.
  const current=Math.round(clock);
  const next=Math.max(0,Math.min(duration,(current+direction)/fps));
  return Math.abs(next-clamped)<1e-7?null:next;
}
