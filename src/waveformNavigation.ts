/** The waveform is a source excerpt, while the editor playhead uses project time. */
export function waveformWindow(duration:number,limit=120):number{
  return Number.isFinite(duration)&&duration>0?Math.min(duration,limit):0;
}
export function waveformSeek(trackStart:number,secondsInWaveform:number,duration:number,limit=120):number|null{
  const visible=waveformWindow(duration,limit);
  if(!Number.isFinite(trackStart)||trackStart<0||!Number.isFinite(secondsInWaveform)||visible===0)return null;
  return Math.round((trackStart+Math.min(visible,Math.max(0,secondsInWaveform)))*100)/100;
}
export function waveformPosition(playhead:number,trackStart:number,duration:number,limit=120):number{
  const visible=waveformWindow(duration,limit);
  if(!Number.isFinite(playhead)||!Number.isFinite(trackStart)||visible===0)return 0;
  return Math.max(0,Math.min(visible,playhead-trackStart));
}
