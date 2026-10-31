import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {ErrorNotice} from "./Ui";
/** Original still inspection shares the stage; compiled playback includes the saved edits. */
export function PhotoClipSource({root,path}:{root:string;path:string}){
  const [source,setSource]=useState(""),[error,setError]=useState("");
  useEffect(()=>{let active=true;setSource("");setError("");void invoke<string>("preview_image",{root,path,maxEdge:1600}).then(value=>{if(active)setSource(value);}).catch(cause=>{if(active)setError(String(cause));});return()=>{active=false;};},[root,path]);
  return error?<ErrorNotice title="The photo preview could not load" error={error}/>:source?<img src={source} alt="Original timeline photo"/>:<p role="status">Loading photo…</p>;
}
