import type {AudioTrack} from "./videoSequence";

/** Non-destructive timeline trims. Keep source offset and the original file unchanged. */
export function trimAudioEndAtPlayhead(track:AudioTrack,playhead:number,sourceDuration:number,projectDuration:number):AudioTrack|null{
  if(![playhead,sourceDuration,projectDuration,track.start_seconds,track.source_in_seconds,track.duration_seconds].every(Number.isFinite))return null;
  const length=Math.round((playhead-track.start_seconds)*100)/100;
  if(length<0.04||playhead>projectDuration+0.000001||track.source_in_seconds+length>sourceDuration+0.000001)return null;
  const fadeIn=Math.min(track.fade_in_seconds,length);
  return {...track,duration_seconds:length,fade_in_seconds:fadeIn,fade_out_seconds:Math.min(track.fade_out_seconds,length-fadeIn)};
}

export function placeAudioAtPlayhead(track:AudioTrack,playhead:number,projectDuration:number):AudioTrack|null{
  if(!Number.isFinite(playhead)||!Number.isFinite(projectDuration))return null;
  const start=Math.round(playhead*100)/100;
  if(start<0||start+track.duration_seconds>projectDuration+0.000001)return null;
  return {...track,start_seconds:start};
}
