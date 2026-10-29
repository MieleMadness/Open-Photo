import {useEffect,useState} from "react";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {ActiveVideoExportError,withAppExitProtection} from "./appExit";
import {flushProjects,projectSaveError,subscribeProjectSaves} from "./videoProject";
export function ProjectSaveGuard(){
  const [error,setError]=useState("");
  const [saveError,setSaveError]=useState(projectSaveError);
  const [exportBlocked,setExportBlocked]=useState(false);
  useEffect(()=>subscribeProjectSaves(()=>setSaveError(projectSaveError())),[]);
  useEffect(()=>{
    if(!("__TAURI_INTERNALS__" in window))return;
    let disposed=false;let unlisten:(()=>void)|undefined;
    void getCurrentWindow().onCloseRequested(async event=>{
      event.preventDefault();
      try{
        await withAppExitProtection(()=>getCurrentWindow().destroy());
      }
      catch(cause){if(cause instanceof ActiveVideoExportError)setExportBlocked(true);else setError(String(cause));}
    }).then(stop=>{if(disposed)stop();else unlisten=stop;}).catch(cause=>setError(`Could not protect unsaved video projects: ${String(cause)}`));
    return()=>{disposed=true;unlisten?.();};
  },[]);
  return <>{exportBlocked&&<div className="storage-error" role="alert">An export is still running or starting. Wait for it to finish or cancel it in the export panel before closing.
    <button onClick={()=>setExportBlocked(false)}>Keep app open</button></div>}
  {(error||saveError)&&<div className="storage-error" role="alert">Video project changes need attention: {saveError||error}
    <button onClick={()=>{void flushProjects(true).then(()=>setError("")).catch(cause=>setError(String(cause)));}}>Retry project saves</button>
    <button onClick={()=>{void withAppExitProtection(()=>getCurrentWindow().destroy(),true).catch(cause=>{if(cause instanceof ActiveVideoExportError)setExportBlocked(true);else setError(String(cause));});}}>Close without saving unsaved changes</button>
  </div>}</>;
}
