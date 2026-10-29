import {useCallback,useEffect,useRef,useState} from "react";
import {getVersion} from "@tauri-apps/api/app";
import {invoke} from "@tauri-apps/api/core";
import {type Update,type DownloadEvent} from "@tauri-apps/plugin-updater";
import {checkForAppUpdate as check} from "./appUpdateService";
import {withAppExitProtection} from "./appExit";

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
  const lifetime=useRef(0);
  const operation=useRef<symbol|null>(null);

  const releaseUpdate=useCallback(async()=>{
    const active=updateRef.current;
    updateRef.current=null;
    if(active)await active.close().catch(()=>undefined);
  },[]);

  const checkNow=useCallback(async()=>{
    if(!desktop||!enabled||operation.current)return;
    const scope=lifetime.current,task=Symbol();operation.current=task;
    setPhase("checking"); setMessage(""); setProgress(null); setDismissed(false);
    try{
      await releaseUpdate();
      if(scope!==lifetime.current)return;
      const update=await check();
      if(scope!==lifetime.current){if(update)await update.close().catch(()=>undefined);return;}
      if(!update){ setAvailableVersion(""); setNotes(""); setPhase("current"); return; }
      if(!isMajorStableRelease(update.version)){
        await update.close();
        if(scope!==lifetime.current)return;
        setAvailableVersion(""); setNotes(""); setPhase("current");
        setMessage("The update service returned a non-major release. Open-Photo ignored it.");
        return;
      }
      updateRef.current=update;
      setAvailableVersion(update.version);
      setNotes(update.body||"");
      setPhase("available");
    }catch(error){ if(scope===lifetime.current){setPhase("error"); setMessage(String(error));} }
    finally{if(operation.current===task)operation.current=null;}
  },[desktop,enabled,releaseUpdate]);

  useEffect(()=>{
    const scope=++lifetime.current;
    if(!desktop)return;
    void Promise.all([
      getVersion().catch(()=>""),
      invoke<ChannelStatus>("updater_channel_status").catch(()=>({enabled:false,policy:"major-only"}))
    ]).then(([version,channel])=>{
      if(scope!==lifetime.current)return;
      setCurrentVersion(version); setEnabled(channel.enabled); setPhase(channel.enabled?"idle":"disabled");
    });
    return()=>{lifetime.current++;operation.current=null;void releaseUpdate();};
  },[desktop,releaseUpdate]);

  useEffect(()=>{
    if(!desktop||!enabled||!currentVersion)return;
    const key=`openphoto.updater.checked.${currentVersion}`;
    // Mark a check only when it actually starts. StrictMode cleanup cancels its timer.
    try{if(sessionStorage.getItem(key)==="true")return;}catch{/* Checking still works when browser storage is unavailable. */}
    const timer=window.setTimeout(()=>{
      if(operation.current)return;
      try{sessionStorage.setItem(key,"true");}catch{/* No storage is required for updates. */}
      void checkNow();
    },1200);
    return()=>window.clearTimeout(timer);
  },[desktop,enabled,currentVersion,checkNow]);

  const download=useCallback(async()=>{
    const update=updateRef.current;
    if(!update||phase!=="available"||operation.current)return;
    const scope=lifetime.current,task=Symbol();operation.current=task;
    setPhase("downloading"); setProgress(0); setMessage("");
    let downloaded=0; let total=0;
    try{
      await update.download((event:DownloadEvent)=>{
        if(scope!==lifetime.current)return;
        if(event.event==="Started"){ total=event.data.contentLength||0; setProgress(total?0:null); }
        else if(event.event==="Progress"){ downloaded+=event.data.chunkLength; setProgress(total?Math.min(1,downloaded/total):null); }
        else if(event.event==="Finished"){ setProgress(1); }
      });
      if(scope===lifetime.current)setPhase("ready");
    }catch(error){ if(scope===lifetime.current){setPhase("error"); setMessage(String(error));} }
    finally{
      // Downloaded bytes get their own native resource only after download resolves.
      // Cleanup may already have closed the update while that request was pending.
      if(scope!==lifetime.current)await update.close().catch(()=>undefined);
      if(operation.current===task)operation.current=null;
    }
  },[phase]);

  const install=useCallback(async()=>{
    const update=updateRef.current;
    if(!update||phase!=="ready"||operation.current)return;
    const scope=lifetime.current,task=Symbol();operation.current=task;
    setPhase("installing"); setMessage("");
    try{ await withAppExitProtection(()=>{
      if(scope!==lifetime.current)throw Error("The update screen closed before installation started.");
      return update.install({restartAfterInstall:true});
    }); }
    catch(error){ if(scope===lifetime.current){setPhase("ready"); setMessage(String(error));} }
    finally{if(operation.current===task)operation.current=null;}
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
