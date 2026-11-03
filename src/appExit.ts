import {hasStartingVideoExport,refreshVideoExport,setAppClosing} from "./videoExport";
import {discardProjectSaves,flushProjects,hasUnsavedProjects} from "./videoProject";

export class ActiveVideoExportError extends Error {
  constructor(){super("An export is still running or starting. Wait for it to finish or cancel it in the export panel before closing or installing an update.");}
}
let exiting=false;
// Window closure and updater installation must use the same save/export boundary.
export async function withAppExitProtection(action:()=>Promise<unknown>,discardUnsaved=false){
  if(exiting)throw Error("A close or restart is already in progress.");
  exiting=true;setAppClosing(true);
  try{
    // Capture registration already in flight before awaiting a possibly older status.
    if(hasStartingVideoExport())throw new ActiveVideoExportError();
    const status=await refreshVideoExport();
    if(status?.active||hasStartingVideoExport())throw new ActiveVideoExportError();
    if(discardUnsaved)discardProjectSaves();
    else if(hasUnsavedProjects())await flushProjects();
    await action();
  }finally{exiting=false;setAppClosing(false);}
}
