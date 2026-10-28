import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {cancelVideoExport,dismissVideoExport,refreshVideoExport,useVideoExport} from "./videoExport";
import {clockTime} from "./videoTime";
const labels={queued:"Waiting for the video engine",verifying_source:"Verifying original video",encoding:"Encoding MP4",checking_output:"Checking output properties",decoding_output:"Checking every output frame",publishing:"Publishing completed file",cancelling:"Cancelling export",completed:"Export complete",cancelled:"Export cancelled",failed:"Export failed"};
export function VideoExportPanel(){
  const status=useVideoExport();const [error,setError]=useState("");
  useEffect(()=>{if(!("__TAURI_INTERNALS__" in window))return;let active=true;void refreshVideoExport().catch(cause=>{if(active)setError(String(cause));});return()=>{active=false;};},[]);
  useEffect(()=>{if(!status?.active)return;let active=true;const timer=setInterval(()=>{void refreshVideoExport().then(()=>{if(active)setError("");}).catch(cause=>{if(active)setError(String(cause));});},700);return()=>{active=false;clearInterval(timer);};},[status?.active]);
  async function action(task:()=>Promise<unknown>){try{await task();setError("");}catch(cause){setError(String(cause));}}
  if(!status&&!error)return null;
  return <aside className="video-export-panel" aria-label="Video export">
    <strong role="status">{status?labels[status.stage]:"Export status unavailable"}</strong>
    {status&&<><p>{status.title}</p><p className="break-anywhere">{status.destination}</p>
      <progress aria-label="Video export progress" max={1} value={status.fraction??undefined}/>
      <small>{clockTime(status.elapsed_seconds)} elapsed · {(status.bytes/1024/1024).toFixed(1)} MB{status.fraction!==null?` · ${Math.floor(status.fraction*100)}%`:""}</small>
      {status.active&&<p>Export uses a snapshot of the saved project. You can keep editing.</p>}
      {status.error&&<p role="alert">{status.stage==="cancelled"?"Export cancelled. No final file was published.":status.error}</p>}
      {status.result&&<><p>{status.result.width} × {status.result.height} · {status.result.duration_seconds.toFixed(2)} seconds</p>{status.result.warnings.map(warning=><small key={warning}>{warning}</small>)}</>}
      <div className="video-transport">
        {status.active?<button className="secondary-button" disabled={!status.can_cancel} onClick={()=>void action(()=>cancelVideoExport(status.id))}>Cancel video export</button>:<>
          {status.result&&<button className="secondary-button" onClick={()=>void action(()=>invoke("open_video_export",{id:status.id}))}>Open exported video</button>}
          <button className="secondary-button" onClick={()=>void action(()=>dismissVideoExport(status.id))}>Dismiss export</button>
        </>}
      </div>
    </>}
    {error&&<div role="alert">{error}<button className="secondary-button" onClick={()=>void action(refreshVideoExport)}>Retry export status</button></div>}
  </aside>;
}
