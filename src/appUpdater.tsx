import {useCallback,useEffect,useRef,useState} from "react";
import {getVersion} from "@tauri-apps/api/app";
import {invoke} from "@tauri-apps/api/core";
import {check,type Update,type DownloadEvent} from "@tauri-apps/plugin-updater";

type ChannelStatus={enabled:boolean;policy:string};
type Phase="disabled"|"idle"|"checking"|"current"|"available"|"downloading"|"ready"|"installing"|"error";

export function isMajorStableRelease(version:string):boolean {
  return /^[1-9]\d*\.0\.0$/.test(version.trim().replace(/^v/,""));
}

export type AppUpdater={
  enabled:boolean; phase:Phase; currentVersion:string; availableVersion:string; notes:string;
  progress:number|null; message:string; dismissed:boolean;
  checkNow:()=>Promise<void>; download:()=>Promise<void>; install:()=>Promise<void>; dismiss:()=>void;
};

export function useAppUpdater(desktop:boolean):AppUpdater {
  const [enabled,setEnabled]=useState(false);
  const [phase,setPhase]=useState<Phase>("disabled");
  const [currentVersion,setCurrentVersion]=useState("");
  const [availableVersion,setAvailableVersion]=useState("");
  const [notes,setNotes]=useState("");
  const [progress,setProgress]=useState<number|null>(null);
  const [message,setMessage]=useState("");
  const [dismissed,setDismissed]=useState(false);
  const updateRef=useRef<Update|null>(null);

  const releaseUpdate=useCallback(async()=>{
    const active=updateRef.current;
    updateRef.current=null;
    if(active)await active.close().catch(()=>undefined);
  },[]);

  const checkNow=useCallback(async()=>{
    if(!desktop||!enabled)return;
    setPhase("checking"); setMessage(""); setProgress(null); setDismissed(false);
    try{
      await releaseUpdate();
      const update=await check({timeout:15_000});
      if(!update){ setAvailableVersion(""); setNotes(""); setPhase("current"); return; }
      if(!isMajorStableRelease(update.version)){
        await update.close();
        setAvailableVersion(""); setNotes(""); setPhase("current");
        setMessage("The update service returned a non-major release. Open-Photo ignored it.");
        return;
      }
      updateRef.current=update;
      setAvailableVersion(update.version);
      setNotes(update.body||"");
      setPhase("available");
    }catch(error){ setPhase("error"); setMessage(String(error)); }
  },[desktop,enabled,releaseUpdate]);

  useEffect(()=>{
    let active=true;
    if(!desktop)return;
    void Promise.all([
      getVersion().catch(()=>""),
      invoke<ChannelStatus>("updater_channel_status").catch(()=>({enabled:false,policy:"major-only"}))
    ]).then(([version,channel])=>{
      if(!active)return;
      setCurrentVersion(version); setEnabled(channel.enabled); setPhase(channel.enabled?"idle":"disabled");
      if(channel.enabled){
        const key=`openphoto.updater.checked.${version}`;
        if(sessionStorage.getItem(key)!=="true"){
          sessionStorage.setItem(key,"true");
          window.setTimeout(()=>{if(active)void checkNow();},1200);
        }
      }
    });
    return()=>{active=false; void releaseUpdate();};
  },[desktop,checkNow,releaseUpdate]);

  const download=useCallback(async()=>{
    const update=updateRef.current;
    if(!update||phase!=="available")return;
    setPhase("downloading"); setProgress(0); setMessage("");
    let downloaded=0; let total=0;
    try{
      await update.download((event:DownloadEvent)=>{
        if(event.event==="Started"){ total=event.data.contentLength||0; setProgress(total?0:null); }
        else if(event.event==="Progress"){ downloaded+=event.data.chunkLength; setProgress(total?Math.min(1,downloaded/total):null); }
        else if(event.event==="Finished"){ setProgress(1); }
      },{timeout:120_000});
      setPhase("ready");
    }catch(error){ setPhase("error"); setMessage(String(error)); }
  },[phase]);

  const install=useCallback(async()=>{
    const update=updateRef.current;
    if(!update||phase!=="ready")return;
    setPhase("installing"); setMessage("");
    try{ await update.install({restartAfterInstall:true}); }
    catch(error){ setPhase("error"); setMessage(String(error)); }
  },[phase]);

  return {enabled,phase,currentVersion,availableVersion,notes,progress,message,dismissed,
    checkNow,download,install,dismiss:()=>setDismissed(true)};
}

export function UpdateToast({updater,onOpenSettings}:{updater:AppUpdater;onOpenSettings:()=>void}){
  const visible=!updater.dismissed&&["available","downloading","ready","error"].includes(updater.phase);
  if(!visible)return null;
  const title=updater.phase==="ready"?"Update ready":updater.phase==="downloading"?"Downloading update":updater.phase==="error"?"Update check needs attention":`Open-Photo ${updater.availableVersion} is available`;
  return <aside className="update-toast" aria-live="polite" aria-label="Open-Photo update">
    <div><strong>{title}</strong>
      {updater.phase==="available"&&<p>Stable major release available. Your current work is not interrupted.</p>}
      {updater.phase==="downloading"&&<><progress max={1} value={updater.progress??undefined}/><small>{updater.progress===null?"Downloading…":`${Math.round(updater.progress*100)}%`}</small></>}
      {updater.phase==="ready"&&<p>The update is downloaded. Finish or save active work, then restart and install from Settings.</p>}
      {updater.phase==="error"&&<p>{updater.message}</p>}
    </div>
    <div className="update-actions"><button className="secondary-button" onClick={updater.dismiss}>Later</button><button className="primary-button" onClick={onOpenSettings}>View update</button></div>
  </aside>;
}

export function UpdateSettingsCard({updater}:{updater:AppUpdater}){
  return <div className="settings-card update-settings-card">
    <div>
      <h2>Open-Photo updates</h2>
      <p>Installed users receive only tested major releases published from main: 1.0.0, 2.0.0, 3.0.0, and so on. Minor, patch, preview, and development versions are never offered through this channel.</p>
      <p className="update-version-line">Installed version: <strong>{updater.currentVersion||"Unknown"}</strong>{updater.availableVersion&&<> · Available: <strong>{updater.availableVersion}</strong></>}</p>
      {updater.notes&&<details><summary>What’s new</summary><p className="update-notes">{updater.notes}</p></details>}
      {updater.message&&<p role={updater.phase==="error"?"alert":"status"}>{updater.message}</p>}
      {!updater.enabled&&<small>This development build is not connected to the public stable updater. Signed public builds enable it automatically.</small>}
      {updater.phase==="ready"&&<small>Installing on Windows closes Open-Photo, runs the signed updater, then relaunches the app. Finish or save active edits and exports first.</small>}
    </div>
    <div className="update-card-actions">
      {updater.enabled&&["idle","current","error"].includes(updater.phase)&&<button className="secondary-button" onClick={()=>void updater.checkNow()}>Check for updates</button>}
      {updater.phase==="checking"&&<button className="secondary-button" disabled>Checking…</button>}
      {updater.phase==="available"&&<button className="primary-button" onClick={()=>void updater.download()}>Download {updater.availableVersion}</button>}
      {updater.phase==="downloading"&&<div className="update-progress"><progress max={1} value={updater.progress??undefined}/><span>{updater.progress===null?"Downloading…":`${Math.round(updater.progress*100)}%`}</span></div>}
      {updater.phase==="ready"&&<button className="primary-button" onClick={()=>void updater.install()}>Restart &amp; install</button>}
      {updater.phase==="installing"&&<button className="primary-button" disabled>Starting installer…</button>}
      {updater.phase==="current"&&<span className="safety-pill">Up to date</span>}
      {!updater.enabled&&<span className="readonly-chip">Development channel</span>}
    </div>
  </div>;
}
