import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";

export type VideoInfo={duration_seconds:number;codec:string;width:number;height:number;sample_aspect_ratio:string|null;frame_rate:string|null;nominal_frame_rate:string|null;time_base:string|null;rotation_degrees:number;hdr:boolean;color_transfer:string|null;audio:{stream:number;codec:string;sample_rate:string|null;channels:number|null;channel_layout:string|null}[];warnings:string[]};

export function VideoDetails({root,path,onInfo}:{root:string;path:string;onInfo?:(value:VideoInfo|null)=>void}){
  const [info,setInfo]=useState<VideoInfo|null>(null);
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const [refresh,setRefresh]=useState(0);
  useEffect(()=>{
    let active=true;setInfo(null);setError("");setBusy(true);onInfo?.(null);
    void invoke<VideoInfo>("inspect_video",{root,path})
      .then(value=>{if(active){setInfo(value);onInfo?.(value)}})
      .catch(reason=>{if(active)setError(String(reason))})
      .finally(()=>{if(active)setBusy(false)});
    return()=>{active=false};
  },[root,path,refresh,onInfo]);
  return <section className="video-details" aria-label="Video details">
    <h3>Video details</h3>
    {busy&&<p role="status">Reading video information…</p>}
    {error&&<div role="alert"><p>{error}</p><button className="secondary-button" onClick={()=>setRefresh(value=>value+1)}>Retry video inspection</button></div>}
    {info&&<><dl>
      <dt>Duration</dt><dd>{info.duration_seconds.toFixed(2)} seconds</dd>
      <dt>Dimensions</dt><dd>{info.width} × {info.height}</dd>
      <dt>Video codec</dt><dd>{info.codec.toUpperCase()}</dd>
      <dt>Average frame rate</dt><dd>{info.frame_rate||"Not reported"}{info.frame_rate&&` (${rate(info.frame_rate)} fps)`}</dd>
      <dt>Rotation metadata</dt><dd>{info.rotation_degrees}°</dd>
      <dt>Color range</dt><dd>{info.hdr?"HDR":"SDR / unspecified"}{info.color_transfer?` · ${info.color_transfer}`:""}</dd>
      <dt>Audio</dt><dd>{info.audio.length?info.audio.map(track=>`${track.codec.toUpperCase()}${track.channels?` · ${track.channels} channels`:""}`).join("; "):"No audio track"}</dd>
    </dl>{info.warnings.map(warning=><p className="results-note" key={warning}>{warning}</p>)}</>}
  </section>;
}

function rate(value:string){const[numerator,denominator]=value.split("/").map(Number);return (numerator/denominator).toFixed(3);}
