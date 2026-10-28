import {useEffect,useState} from "react";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {discardProjectSaves,flushProjects,hasUnsavedProjects,projectSaveError,subscribeProjectSaves} from "./videoProject";
export function ProjectSaveGuard(){
  const [error,setError]=useState("");
  const [saveError,setSaveError]=useState(projectSaveError);
  useEffect(()=>subscribeProjectSaves(()=>setSaveError(projectSaveError())),[]);
  useEffect(()=>{
    if(!("__TAURI_INTERNALS__" in window))return;
    let disposed=false;let unlisten:(()=>void)|undefined;let closing=false;
    void getCurrentWindow().onCloseRequested(async event=>{
      if(closing||!hasUnsavedProjects())return;
      event.preventDefault();
      try{await flushProjects();closing=true;await getCurrentWindow().close();}
      catch(cause){setError(String(cause));}
    }).then(stop=>{if(disposed)stop();else unlisten=stop;}).catch(cause=>setError(`Could not protect unsaved video projects: ${String(cause)}`));
    return()=>{disposed=true;unlisten?.();};
  },[]);
  return error||saveError?<div className="storage-error" role="alert">Video project changes need attention: {saveError||error}
    <button onClick={()=>{void flushProjects(true).then(()=>setError("")).catch(cause=>setError(String(cause)));}}>Retry project saves</button>
    <button onClick={()=>{discardProjectSaves();void getCurrentWindow().close();}}>Close without saving unsaved changes</button>
  </div>:null;
}
