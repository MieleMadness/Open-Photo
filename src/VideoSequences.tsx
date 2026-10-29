import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {VideoPlayer} from "./VideoPlayer";
import {SequenceSession,type LoadedSequence} from "./videoSequence";
import {exportFilename} from "./videoExport";

const videoFilters=[{name:"Videos",extensions:["mp4","mov","m4v","mkv","webm","avi"]}];
export function VideoSequences({roots,workspaceId}:{roots:string[];workspaceId:string}){
  const [chosen,setChosen]=useState("");const root=roots.includes(chosen)?chosen:roots[0]||"";
  return <><span className="eyebrow">VIDEO PROJECTS</span><h1>Build your next video</h1><p>Keep several videos together in a saved project. Select a video in the media bin to preview its original.</p>
    {!roots.length?<p>Add a folder to your library to start a video project.</p>:<><label className="sequence-root">Project folder<select aria-label="Video project folder" value={root} onChange={e=>setChosen(e.target.value)}>{roots.map(r=><option key={r} value={r}>{r}</option>)}</select></label><SequenceFolder key={`${workspaceId}:${root}`} root={root}/></>}
  </>;
}
function SequenceFolder({root}:{root:string}){
  const [session,setSession]=useState<SequenceSession|null>(null),[loaded,setLoaded]=useState<LoadedSequence|null>(null);
  const owner=useRef<SequenceSession|null>(null),generation=useRef(0),working=useRef(false);
  const [recent,setRecent]=useState<{id:string;title:string}[]>([]),[selected,setSelected]=useState("");
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[message,setMessage]=useState("");const [,refresh]=useState(0);
  const project=session?.history.present;
  useEffect(()=>{const token=++generation.current;void invoke<unknown>("recent_video_sequences",{root}).then(rows=>{if(token===generation.current)setRecent(Array.isArray(rows)?rows:[]);}).catch(e=>{if(token===generation.current)setError(String(e));});return()=>{generation.current++;owner.current?.release();};},[root]);
  useEffect(()=>session?.subscribe(()=>refresh(n=>n+1)),[session]);
  async function run(task:(current:()=>boolean)=>Promise<void>,flush=true){
    if(working.current)return;working.current=true;setBusy(true);setError("");setMessage("");const token=generation.current,current=()=>token===generation.current;
    try{if(flush)await owner.current?.flush();if(current())await task(current);}catch(e){if(current())setError(String(e));}finally{working.current=false;if(current())setBusy(false);}
  }
  function adopt(value:LoadedSequence,current:()=>boolean){
    if(!current())return;owner.current?.release();const next=new SequenceSession(root,value);owner.current=next;setSession(next);setLoaded(value);
    setSelected(id=>value.project.assets.some(a=>a.id===id)?id:value.project.assets[0].id);
    void invoke<unknown>("recent_video_sequences",{root}).then(rows=>{if(current())setRecent(Array.isArray(rows)?rows:[]);}).catch(e=>{if(current())setError(String(e));});
  }
  async function chooseVideos(current:()=>boolean){const files=await open({multiple:true,defaultPath:root,filters:videoFilters});return current()?(Array.isArray(files)?files:typeof files==="string"?[files]:[]):[];}
  const asset=project?.assets.find(a=>a.id===selected),source=loaded?.sources.find(s=>s.asset_id===selected);
  return <section aria-label="Multi-video projects" className="sequence-workspace">
    <div className="video-transport">
      <button className="primary-button" disabled={busy} onClick={()=>void run(async current=>{const paths=await chooseVideos(current);if(paths.length)adopt(await invoke<LoadedSequence>("create_video_sequence",{root,paths}),current);})}>New multi-video project…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const path=await open({multiple:false,filters:[{name:"Open-Photo video project",extensions:["json"]}]});if(typeof path==="string"&&current()){adopt(await invoke<LoadedSequence>("import_video_sequence",{root,path}),current);if(current())setMessage("Opened a new project copy. Existing single-video drafts remain available in the Library.");}})}>Open project file…</button>
      {!!recent.length&&<label>Saved projects<select aria-label="Saved multi-video projects" disabled={busy} value="" onChange={e=>{const id=e.target.value;if(id)void run(async current=>adopt(await invoke<LoadedSequence>("load_video_sequence",{root,id}),current));}}><option value="">Choose a project…</option>{recent.map(p=><option key={p.id} value={p.id}>{p.title}</option>)}</select></label>}
    </div>
    {busy&&<p role="status">Verifying videos and saving the project…</p>}
    {(error||session?.error)&&<div role="alert" className="error">{error||session?.error}{session?.error&&<>
      <button className="secondary-button" disabled={busy} onClick={()=>{setError("");session.retry();}}>Retry project save</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const destination=await save({defaultPath:"unsaved-video-sequence.json"});if(destination&&current()){await invoke("write_video_sequence_copy",{project:session.history.present,destination});if(current())setMessage("Unsaved project copy saved.");}},false)}>Save unsaved project copy…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const value=await invoke<LoadedSequence>("load_video_sequence",{root,id:session.history.present.id});if(current()){session.discard();adopt(value,current);}},false)}>Discard unsaved changes and reopen draft</button>
    </>}</div>}
    {message&&<p role="status">{message}</p>}
    {project&&session&&<>
      <fieldset disabled={busy} className="sequence-header"><label>Project name<input type="text" aria-label="Multi-video project name" maxLength={200} value={project.title} onChange={e=>{try{session.commit({...project,title:e.target.value});setError("");}catch(cause){setError(String(cause));}}}/></label>
        <p role="status">{session.status==="saved"?"All project changes saved locally.":session.status==="saving"?"Saving project changes…":"Project changes need attention."}</p>
        <div className="video-transport"><button className="secondary-button" onClick={()=>void run(async current=>{const paths=await chooseVideos(current);if(paths.length){const before=project.assets.length;const value=await invoke<LoadedSequence>("add_sequence_videos",{root,id:project.id,paths,expectedRevision:session.revision});adopt(value,current);if(current())setMessage(`${value.project.assets.length-before} videos added. Identical videos already in the bin were skipped.`);}})}>Add videos…</button>
          <button className="secondary-button" onClick={()=>void run(async current=>{const destination=await save({defaultPath:exportFilename(project.title).replace(/\.edited\.mp4$/,".openphoto-sequence.json"),filters:[{name:"Video project",extensions:["json"]}]});if(destination&&current()){await invoke("write_video_sequence_copy",{project:owner.current!.history.present,destination});if(current())setMessage("Project copy saved. Source videos are referenced and remain unchanged.");}})}>Save project copy…</button>
        </div>
      </fieldset>
      <div className="sequence-layout"><section aria-label="Video media bin" className="sequence-bin"><h2>Media bin <small>{project.assets.length} / 50 videos</small></h2><p>Videos in this project folder. Adding a file again does not duplicate it.</p><ul>{project.assets.map(a=>{const status=loaded?.sources.find(s=>s.asset_id===a.id);return <li key={a.id}><button className={selected===a.id?"sequence-asset selected":"sequence-asset"} aria-pressed={selected===a.id} disabled={busy} onClick={()=>setSelected(a.id)}><strong>{a.relative_path.split("/").at(-1)}</strong><small>{a.info.duration_seconds.toFixed(2)} s · {a.info.width} × {a.info.height} · {a.info.audio.length?"Audio":"Silent"}{!status?.path?" · Needs relinking":""}</small></button></li>;})}</ul></section>
        <section className="sequence-preview" aria-label="Selected source preview"><h2>Source preview</h2>{asset&&<><p>{asset.relative_path}</p>{source?.path?<VideoPlayer key={source.path} root={root} path={source.path} autoLoad onOpenExternal={()=>{void invoke("open_video",{root,path:source.path}).catch(e=>setError(String(e)));}}/>:<p role="alert">This video needs relinking: {source?.status}</p>}
          <div className="video-transport"><button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const path=await open({multiple:false,defaultPath:root,filters:videoFilters});if(typeof path==="string"&&current())adopt(await invoke<LoadedSequence>("relink_sequence_video",{root,id:project.id,assetId:asset.id,path,expectedRevision:session.revision}),current);})}>Relink original video…</button>
            <button className="secondary-button" disabled={busy||project.assets.length===1} onClick={()=>void run(async current=>adopt(await invoke<LoadedSequence>("remove_sequence_video",{root,id:project.id,assetId:asset.id,expectedRevision:session.revision}),current))}>Remove from project</button></div><small>Removing a video from this project keeps its original file unchanged.</small>
        </>}</section></div>
    </>}
  </section>;
}
