import type {AudioTrack} from "./videoSequence";

export type AudioTrimEdge="start"|"end";
export const MIN_AUDIO_TRIM_SECONDS=0.04;

/** Trim an audio layer without changing source files or moving its opposite timeline boundary.
 * Start trims advance both the project start and source in-point; end trims change only duration.
 * Negative deltas extend into available source, never outside project or source bounds.
 */
export function trimAudioEdge(track:AudioTrack,edge:AudioTrimEdge,deltaSeconds:number,sourceDuration:number,projectDuration:number):AudioTrack|null{
  const numbers=[deltaSeconds,sourceDuration,projectDuration,track.start_seconds,track.source_in_seconds,track.duration_seconds,track.fade_in_seconds,track.fade_out_seconds];
  if(!numbers.every(Number.isFinite)||sourceDuration<=0||projectDuration<=0||track.duration_seconds<MIN_AUDIO_TRIM_SECONDS)return null;
  const delta=Math.round(deltaSeconds*100)/100;
  if(delta===0)return null;
  const originalEnd=track.start_seconds+track.duration_seconds;
  if(edge==="start"){
    const lower=Math.max(-track.start_seconds,-track.source_in_seconds);
    const upper=track.duration_seconds-MIN_AUDIO_TRIM_SECONDS;
    const applied=Math.max(lower,Math.min(upper,delta));
    if(Math.abs(applied)<0.000001)return null;
    const duration=Math.round((track.duration_seconds-applied)*100)/100;
    const start=Math.round((track.start_seconds+applied)*100)/100;
    const sourceIn=Math.round((track.source_in_seconds+applied)*100)/100;
    if(start<0||sourceIn<0||duration<MIN_AUDIO_TRIM_SECONDS-0.000001||originalEnd>projectDuration+0.000001||sourceIn+duration>sourceDuration+0.000001)return null;
    const fadeIn=Math.min(track.fade_in_seconds,duration);
    return {...track,start_seconds:start,source_in_seconds:sourceIn,duration_seconds:duration,fade_in_seconds:fadeIn,fade_out_seconds:Math.min(track.fade_out_seconds,duration-fadeIn)};
  }
  if(edge==="end"){
    const lower=MIN_AUDIO_TRIM_SECONDS-track.duration_seconds;
    const upper=Math.min(sourceDuration-track.source_in_seconds-track.duration_seconds,projectDuration-originalEnd);
    const applied=Math.max(lower,Math.min(upper,delta));
    if(Math.abs(applied)<0.000001)return null;
    const duration=Math.round((track.duration_seconds+applied)*100)/100;
    if(duration<MIN_AUDIO_TRIM_SECONDS-0.000001)return null;
    const fadeIn=Math.min(track.fade_in_seconds,duration);
    return {...track,duration_seconds:duration,fade_in_seconds:fadeIn,fade_out_seconds:Math.min(track.fade_out_seconds,duration-fadeIn)};
  }
  return null;
}
