import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {waveformPosition,waveformSeek,waveformWindow} from "./waveformNavigation";

type Props={
  workspaceId:string;
  projectId:string;
  assetId:string;
  revision:number;
  sourceIn:number;
  duration:number;
  ready:boolean;
  trackStart:number;
  playhead:number;
  onSeek:(seconds:number)=>void;
};

/** Real, server-measured peaks. Never synthesize decorative waveform amplitudes. */
export function AudioWaveform({workspaceId,projectId,assetId,revision,sourceIn,duration,ready,trackStart,playhead,onSeek}:Props){
  const [peaks,setPeaks]=useState<number[]|null>(null);
  const [error,setError]=useState("");
  useEffect(()=>{
    let cancelled=false;
    setPeaks(null);
    setError("");
    if(!ready||revision<1||duration<0.04)return()=>{cancelled=true;};
    void invoke<number[]>("workspace_audio_waveform",{
      workspaceId,id:projectId,assetId,expectedRevision:revision,
      sourceInSeconds:sourceIn,durationSeconds:duration
    }).then(values=>{
      if(!cancelled)setPeaks(values.length===64?values:null);
    }).catch(()=>{
      if(!cancelled)setError("Waveform unavailable — audio editing still works.");
    });
    return()=>{cancelled=true;};
  },[workspaceId,projectId,assetId,revision,sourceIn,duration,ready]);
  const visibleSeconds=waveformWindow(duration);
  return <div className="sequence-waveform-container" role="group" aria-label="Measured audio waveform">
    {peaks?<div className="sequence-waveform-seek">
      <div className="sequence-waveform" aria-hidden="true">{peaks.map((peak,index)=><span key={index} className="sequence-waveform-peak" style={{height:`${Math.max(2,Math.min(100,peak))}%`}}/>)}</div>
      <input
        className="sequence-waveform-slider"
        type="range"
        aria-label="Seek project using audio waveform"
        aria-valuetext={`${(trackStart+waveformPosition(playhead,trackStart,duration)).toFixed(2)} seconds on project timeline`}
        min={0}
        max={visibleSeconds}
        step="0.01"
        value={waveformPosition(playhead,trackStart,duration)}
        onChange={event=>{
          const target=waveformSeek(trackStart,Number(event.target.value),duration);
          if(target!==null)onSeek(target);
        }}
      />
    </div>:<p className="sequence-waveform-fallback" role="status">{!ready?"Relink this file to view its waveform.":error||"Measuring waveform…"}</p>}
    {peaks&&<small>Drag or use arrow keys to seek; use End at playhead to trim.{duration>120?" First 120 seconds shown.":""}</small>}
  </div>;
}
