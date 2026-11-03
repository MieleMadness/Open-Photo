import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";

type Props={
  workspaceId:string;
  projectId:string;
  assetId:string;
  revision:number;
  sourceIn:number;
  duration:number;
  ready:boolean;
};

/** Real, server-measured peaks. Never synthesize decorative waveform amplitudes. */
export function AudioWaveform({workspaceId,projectId,assetId,revision,sourceIn,duration,ready}:Props){
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
  return <div className="sequence-waveform-container" role="group" aria-label="Measured audio waveform">
    {peaks?<div className="sequence-waveform" aria-hidden="true">{peaks.map((peak,index)=><span key={index} className="sequence-waveform-peak" style={{height:`${Math.max(2,Math.min(100,peak))}%`}}/>)}</div>:<p className="sequence-waveform-fallback" role="status">{!ready?"Relink this file to view its waveform.":error||"Measuring waveform…"}</p>}
    {peaks&&duration>120&&<small>First 120 seconds shown. Timing controls affect the full clip.</small>}
  </div>;
}
