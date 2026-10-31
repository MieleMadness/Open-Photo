import {useSyncExternalStore} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {VideoProject} from "./videoProject";
export type ExportSize="project"|"hd1080"|"uhd4k";
/** The display mirrors native resolution caps; native validation remains authoritative. */
export function exportCanvas(canvas:VideoProject["output"],preset:ExportSize){
  if(preset==="project")return {...canvas};let [width,height]=preset==="hd1080"?[1920,1080]:[3840,2160];if(canvas.height>canvas.width)[width,height]=[height,width];
  const ratio=Math.min(width/canvas.width,height/canvas.height,1);return {...canvas,width:Math.max(16,Math.floor(canvas.width*ratio/2)*2),height:Math.max(16,Math.floor(canvas.height*ratio/2)*2)};
}
export type ExportStatus={id:string;title:string;destination:string;active:boolean;can_cancel:boolean;stage:"queued"|"verifying_source"|"encoding"|"checking_output"|"decoding_output"|"publishing"|"cancelling"|"completed"|"cancelled"|"failed";fraction:number|null;bytes:number;elapsed_seconds:number;error:string|null;result:{path:string;bytes:number;width:number;height:number;duration_seconds:number;warnings:string[]}|null};
let snapshot:ExportStatus|null=null;
let starting=0,appClosing=false;
let epoch=0,issued=0,applied=0;
export const hasStartingVideoExport=()=>starting>0;
export const setAppClosing=(value:boolean)=>{appClosing=value;};
const listeners=new Set<()=>void>();
const subscribe=(listener:()=>void)=>{listeners.add(listener);return()=>{listeners.delete(listener);};};
function update(value:ExportStatus|null){snapshot=value;listeners.forEach(listener=>listener());}
export const useVideoExport=()=>useSyncExternalStore(subscribe,()=>snapshot,()=>null);
export async function refreshVideoExport(){const generation=epoch,request=++issued;const status=await invoke<ExportStatus|null>("video_export_status");if(generation===epoch&&request>applied){applied=request;update(status||null);}return snapshot;}
export async function startVideoExport(root:string,project:VideoProject,options:{destination:string;size:ExportSize;acknowledge_first_audio:boolean}){
  return startExport("start_video_export",{root,project,options},project.title,options.destination);
}
export async function startWorkspaceVideoExport(workspaceId:string,id:string,expectedRevision:number,title:string,options:{destination:string;size:ExportSize;acknowledge_first_audio:boolean}){
  return startExport("start_workspace_video_export",{workspaceId,id,expectedRevision,options},title,options.destination);
}
async function startExport(command:string,args:Record<string,unknown>,title:string,destination:string){
  if(appClosing)throw Error("The app is closing; start export after keeping it open.");
  starting++;
  try{
    const id=await invoke<string>(command,args);
    epoch++;
    update({id,title,destination,active:true,can_cancel:true,stage:"queued",fraction:null,bytes:0,elapsed_seconds:0,error:null,result:null});
    void refreshVideoExport().catch(()=>{});return id;
  }finally{starting--;}
}
export async function cancelVideoExport(id:string){await invoke("cancel_video_export",{id});await refreshVideoExport();}
export async function dismissVideoExport(id:string){await invoke("dismiss_video_export",{id});await refreshVideoExport();}
export function exportFilename(title:string){let name=title.replace(/[<>:"/\\|?*\u0000-\u001f]/g,"_").replace(/[. ]+$/g,"")||"video";if(/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name))name=`video-${name}`;return `${name}.edited.mp4`;}

