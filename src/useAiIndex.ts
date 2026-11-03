import {useCallback,useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
export type IndexCounts={indexed:number;pending:number;failed:number;partial:number;unsupported:number;excluded:number};
export type IndexReport={counts:IndexCounts;folders:{path:string;excluded:boolean;inherited:boolean;note?:string|null;counts:IndexCounts}[];failures:{root:string;path:string;service:string;error:string;attempted:number|null}[];failure_total:number;running:boolean;error:string|null;revision:number;generation:number};
export function useAiIndex({workspace,enabled,device,snapshot,desktop,onProgress}:{workspace:string;enabled:boolean;device:string;snapshot:unknown;desktop:boolean;onProgress:()=>void}){
 const [pollError,setPollError]=useState("");
 const [report,setReport]=useState<{workspace:string;value:IndexReport}|null>(null),[error,setError]=useState("");
 const applied=useRef({workspace:"",generation:-1,revision:-1});
 const latest=useRef(workspace),progress=useRef(onProgress),version=useRef(-1);latest.current=workspace;progress.current=onProgress;
 const refresh=useCallback(async()=>{if(!desktop||!workspace)return;const value=await invoke<IndexReport>("ai_index_report",{workspaceId:workspace});if(latest.current===workspace&&value&&Array.isArray(value.folders)){if(applied.current.workspace===workspace&&(value.generation<applied.current.generation||value.generation===applied.current.generation&&value.revision<applied.current.revision))return;applied.current={workspace,generation:value.generation,revision:value.revision};setPollError("");setReport({workspace,value});if(value.revision!==version.current){version.current=value.revision;progress.current();}}},[workspace,desktop]);
 useEffect(()=>{if(!desktop)return;let active=true;setError("");void invoke("pause_ai_index").catch(e=>{if(active)setError(String(e));});void flushSettings(["openphoto.workspaces","openphoto.ai.optin","openphoto.ai.device"]).then(()=>{if(active)return invoke("configure_ai_index",{workspaceId:workspace,enabled:enabled&&!!snapshot,device});}).then(()=>{if(active)return refresh();}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[workspace,enabled,device,snapshot,desktop,refresh]);
 useEffect(()=>{if(!desktop||!workspace)return;let active=true,timer:ReturnType<typeof setTimeout>;const poll=async()=>{try{await refresh();}catch(e){if(active)setPollError(String(e));}if(active)timer=setTimeout(()=>void poll(),1500);};void poll();return()=>{active=false;clearTimeout(timer);};},[workspace,desktop,refresh]);
 async function exclude(folder:string,excluded:boolean){await invoke("ai_index_exclude",{workspaceId:workspace,folder,excluded});await refresh();progress.current();}
 async function retry(path?:string){await invoke("retry_ai_index",{workspaceId:workspace,path:path||null});await refresh();}
 async function failures(query:string,offset:number){const value=await invoke<IndexReport>("ai_index_report",{workspaceId:workspace,failureQuery:query,failureOffset:offset});return {items:value.failures,total:value.failure_total};}
 return {failures,report:report?.workspace===workspace?report.value:null,error:error||pollError,exclude,retry,refresh};
}

export function isAiExcluded(path:string,folders:IndexReport["folders"]|undefined){const normalize=(value:string)=>value.replace(/\\/g,"/").replace(/^\/\/\?\//,"").replace(/\/$/,"").toLocaleLowerCase();const value=normalize(path);return !!folders?.some(folder=>folder.excluded&&(value===normalize(folder.path)||value.startsWith(normalize(folder.path)+"/")));}
