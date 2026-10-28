import {useCallback,useEffect,useRef,useState} from "react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {VideoPlayer} from "./VideoPlayer";
import {ProjectSession,defaultEdits,defaultOutput,cropForAspect,type LoadedProject,type VideoProject,type Edits} from "./videoProject";
import {clockTime,markSeconds,markTime} from "./videoTime";
import {exportFilename,startVideoExport,useVideoExport,type ExportSize} from "./videoExport";

function NumberField({label,value,min,max,step=1,onCommit}:{label:string;value:number;min:number;max:number;step?:number;onCommit:(value:number)=>void}){
  const [draft,setDraft]=useState(String(value));
  useEffect(()=>setDraft(String(value)),[value]);
  return <label>{label}<input aria-label={label} type="number" value={draft} min={min} max={max} step={step} onChange={event=>setDraft(event.target.value)} onBlur={()=>{const number=Number(draft);if(draft.trim()&&Number.isFinite(number)&&number>=min&&number<=max)onCommit(number);setDraft(String(value));}} onKeyDown={event=>{if(event.key==="Enter")event.currentTarget.blur();}}/></label>;
}
export function VideoWorkspace({root,path,onOpenExternal}:{root:string;path:string;onOpenExternal:()=>void}){
  const [session,setSession]=useState<ProjectSession|null>(null);
  const sessionRef=useRef<ProjectSession|null>(null);
  const [loaded,setLoaded]=useState<LoadedProject|null>(null);
  const [recent,setRecent]=useState<{id:string;title:string}[]>([]);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [,refresh]=useState(0);
  const alive=useRef(true);
  const [previewSrc,setPreviewSrc]=useState("");
  const [previewBusy,setPreviewBusy]=useState(false);
  const [previewWarnings,setPreviewWarnings]=useState<string[]>([]);
  const cancelPreview=useRef(()=>{});
  const project=session?.history.present;
  const exportStatus=useVideoExport();
  const [exportSize,setExportSize]=useState<ExportSize>("project");
  const [acknowledgeAudio,setAcknowledgeAudio]=useState(false);
  useEffect(()=>setAcknowledgeAudio(false),[project?.id]);
  const updateRecent=useCallback(()=>{void invoke<unknown>("recent_video_projects",{root}).then(rows=>{if(alive.current)setRecent(Array.isArray(rows)?rows:[]);}).catch(cause=>{if(alive.current)setError(String(cause));});},[root]);
  useEffect(()=>{alive.current=true;updateRecent();return()=>{alive.current=false;sessionRef.current?.release();cancelPreview.current();};},[updateRecent]);
  useEffect(()=>session?.subscribe(()=>{if(alive.current)refresh(v=>v+1);}),[session]);
  const stopPreview=useCallback(()=>{cancelPreview.current();setPreviewSrc("");setPreviewBusy(false);setPreviewWarnings([]);},[]);
  useEffect(()=>{stopPreview();},[project,stopPreview]);
  async function action(task:()=>Promise<void>){setBusy(true);setError("");setMessage("");try{await task();}catch(cause){if(alive.current)setError(String(cause));}finally{if(alive.current)setBusy(false);}}
  function adopt(value:LoadedProject){
    if(!alive.current)return;
    sessionRef.current?.release();const next=new ProjectSession(root,value);sessionRef.current=next;setSession(next);setLoaded(value);stopPreview();updateRecent();
  }
  async function flush(){await sessionRef.current?.flush();}
  function change(mutator:(copy:VideoProject)=>void){
    if(!session)return;
    try{const next=structuredClone(session.history.present);mutator(next);session.commit(next);setError("");}catch(cause){setError(String(cause));}
  }
  function edits(mutator:(edit:Edits)=>void){change(p=>mutator(p.clips[0].edits));}
  function history(command:"undo"|"redo"){try{session?.commit(command);}catch(cause){setError(String(cause));}}
  async function preview(){
    if(!project||!loaded?.source_path)return;
    const owner=sessionRef.current;await owner?.flush();
    if(!alive.current||owner!==sessionRef.current||!owner)return;
    const target=owner.history.present;
    stopPreview();setError("");setPreviewBusy(true);
    let active=true,id="";let timer:ReturnType<typeof setTimeout>|undefined;
    const cancel=()=>{active=false;clearTimeout(timer);if(id)void invoke("cancel_video_preview",{id}).catch(()=>{});};cancelPreview.current=cancel;
    try{
      id=await invoke<string>("start_video_project_preview",{root,project:target});
      if(!active||!alive.current){void invoke("cancel_video_preview",{id}).catch(()=>{});return;}
      const poll=async()=>{
        try{
          const status=await invoke<{state:string;error:string|null;result:{source:string;warnings:string[]}|null}>("video_preview_status",{id});
          if(!active||!alive.current)return;
          if(status.state==="failed")throw Error(status.error||"Edit preview failed");
          if(status.state==="complete"&&status.result){setPreviewSrc(convertFileSrc(status.result.source));setPreviewWarnings(status.result.warnings);setPreviewBusy(false);return;}
          timer=setTimeout(()=>void poll(),400);
        }catch(cause){if(!active||!alive.current)return;cancel();setError(String(cause));setPreviewBusy(false);}
      };await poll();
    }catch(cause){if(!active||!alive.current)return;cancel();setError(String(cause));setPreviewBusy(false);}
  }
  const c=project?.clips[0],e=c?.edits;
  async function exportVideo(){
    const owner=sessionRef.current;if(!owner||!loaded?.source_path)return;
    await owner.flush();
    const destination=await save({defaultPath:exportFilename(owner.history.present.title),filters:[{name:"MP4 video",extensions:["mp4"]}]});
    if(!destination||!alive.current||owner!==sessionRef.current)return;
    await owner.flush();if(!alive.current||owner!==sessionRef.current)return;
    await startVideoExport(root,structuredClone(owner.history.present),{destination,size:exportSize,acknowledge_first_audio:acknowledgeAudio});
  }
  return <div className="video-workspace">
    {!previewBusy&&!previewSrc&&(!project||loaded?.source_path)&&<VideoPlayer key={loaded?.source_path||path} root={root} path={loaded?.source_path||path} onOpenExternal={project?()=>{void invoke("open_video",{root,path:loaded?.source_path}).catch(cause=>setError(String(cause)));}:onOpenExternal}
      selection={c?{start:c.source_in,end:c.source_out,onChange:(start,end)=>change(p=>{p.clips[0].source_in=start;p.clips[0].source_out=end;})}:undefined}/>}
    {(previewBusy||previewSrc)&&<section aria-label="Edited video preview" className="video-edit-preview">
      {previewBusy?<p role="status">Rendering saved edits… <button className="secondary-button" onClick={stopPreview}>Cancel edit preview</button></p>:<video key={previewSrc} src={previewSrc} controls playsInline aria-label="Edited video" onError={()=>setError("Windows could not play the edited preview. Original playback remains available.")}/>}
      {c&&<small>Preview source interval: {clockTime(markSeconds(c.source_in))} to {clockTime(markSeconds(c.source_out))}. Preview time starts at zero.</small>}
      {previewWarnings.map(warning=><p key={warning}>{warning}</p>)}
      <button className="secondary-button" onClick={stopPreview}>Back to source</button>
    </section>}
    <section className="video-project" aria-label="Video editing project">
      <h3>Video project</h3>
      <div className="video-transport">
        <button className="secondary-button" disabled={busy} onClick={()=>void action(async()=>{await flush();adopt(await invoke<LoadedProject>("create_video_project",{root,path}));})}>New project from selected video</button>
        <button className="secondary-button" disabled={busy} onClick={()=>void action(async()=>{await flush();const file=await open({multiple:false,filters:[{name:"Open-Photo video project",extensions:["json"]}]});if(typeof file==="string")adopt(await invoke<LoadedProject>("import_video_project",{root,path:file}));})}>Open project file…</button>
      </div>
      {!!recent.length&&<label>Recent projects<select aria-label="Recent video projects" value="" disabled={busy} onChange={event=>{const id=event.target.value;if(id)void action(async()=>{await flush();adopt(await invoke<LoadedProject>("load_video_project",{root,id}));});}}><option value="">Choose a saved draft…</option>{recent.map(item=><option key={item.id} value={item.id}>{item.title}</option>)}</select></label>}
      {busy&&<p role="status">Verifying project and source…</p>}
      {project&&c&&e&&<>
        <label>Project name<input aria-label="Video project name" value={project.title} maxLength={200} onChange={event=>change(p=>{p.title=event.target.value;})}/></label>
        <p className="results-note" role="status">{session.status==="saved"?"All project changes saved locally.":session.status==="saving"?"Saving project changes…":"Project changes have not been saved."}</p>
        {session.error&&<div role="alert">{session.error}<button className="secondary-button" onClick={()=>session.retry()}>Retry project save</button>
          <button className="secondary-button" onClick={()=>void action(async()=>{const value=await invoke<LoadedProject>("load_video_project",{root,id:project.id});session.discard();adopt(value);})}>Discard unsaved changes and reload draft</button>
          <button className="secondary-button" onClick={()=>void action(async()=>{const destination=await save({defaultPath:"unsaved-video-project.json"});if(destination)await invoke("write_video_project_copy",{project:session.history.present,destination});})}>Save unsaved project copy…</button>
        </div>}
        {!loaded?.source_path&&<p role="alert">Source needs relinking: {loaded?.source_status}</p>}
        <div className="video-transport">
          <button className="secondary-button" disabled={!session.history.past.length||busy} onClick={()=>history("undo")}>Undo video edit</button>
          <button className="secondary-button" disabled={!session.history.future.length||busy} onClick={()=>history("redo")}>Redo video edit</button>
          <button className="secondary-button" disabled={busy} onClick={()=>change(p=>{p.clips[0].edits=defaultEdits();p.output=defaultOutput(p.asset.info);p.clips[0].source_in=markTime(0,p.asset.info.time_base);p.clips[0].source_out=markTime(p.asset.info.duration_seconds,p.asset.info.time_base);})}>Reset video edits</button>
          <button className="secondary-button" disabled={busy||previewBusy||!loaded?.source_path||!!project.asset.info.hdr} onClick={()=>void action(preview)}>Preview edits</button>
        </div>
        <div className="video-edit-fields">
          <NumberField label="Trim start (seconds)" value={markSeconds(c.source_in)} min={0} max={project.asset.info.duration_seconds} step={0.001} onCommit={value=>change(p=>{p.clips[0].source_in=markTime(value,p.asset.info.time_base);})}/>
          <NumberField label="Trim end (seconds)" value={markSeconds(c.source_out)} min={0} max={project.asset.info.duration_seconds} step={0.001} onCommit={value=>change(p=>{p.clips[0].source_out=markTime(value,p.asset.info.time_base);})}/>
          <label>Rotation<select aria-label="Video rotation" value={e.quarter_turns} onChange={event=>edits(edit=>{edit.quarter_turns=Number(event.target.value);})}>{[0,1,2,3].map(turn=><option key={turn} value={turn}>{turn*90}° clockwise</option>)}</select></label>
          <label><input type="checkbox" checked={e.flip_horizontal} onChange={event=>edits(edit=>{edit.flip_horizontal=event.target.checked;})}/>Flip horizontally</label>
          <label><input type="checkbox" checked={e.flip_vertical} onChange={event=>edits(edit=>{edit.flip_vertical=event.target.checked;})}/>Flip vertically</label>
          <label><input type="checkbox" checked={e.muted} onChange={event=>edits(edit=>{edit.muted=event.target.checked;})}/>Mute video audio</label>
          <NumberField label="Audio volume (%)" value={e.gain_percent} min={0} max={200} onCommit={value=>edits(edit=>{edit.gain_percent=value;})}/>
          <label>Canvas<select aria-label="Video canvas" value={`${project.output.width}x${project.output.height}`} onChange={event=>change(p=>{const [width,height]=event.target.value.split("x").map(Number);p.output.width=width;p.output.height=height;})}><option value="1920x1080">Landscape 16:9</option><option value="1080x1920">Portrait 9:16</option><option value="1080x1080">Square 1:1</option>{!["1920x1080","1080x1920","1080x1080"].includes(`${project.output.width}x${project.output.height}`)&&<option value={`${project.output.width}x${project.output.height}`}>Imported canvas</option>}</select></label>
          <label>Canvas placement<select aria-label="Video canvas placement" value={project.output.fit} onChange={event=>change(p=>{p.output.fit=event.target.value as "fit"|"fill";})}><option value="fit">Fit, with black borders</option><option value="fill">Fill, with centered crop</option></select></label>
        </div>
        <fieldset><legend>Crop before rotation (% of displayed source)</legend>
          <div className="video-edit-fields">{(["x","y","width","height"] as const).map(field=><NumberField key={field} label={`Crop ${field} (%)`} value={e.crop[field]/100} min={field==="width"||field==="height"?1:0} max={100} step={0.01} onCommit={value=>edits(edit=>{edit.crop[field]=Math.round(value*100);})}/>)}</div>
          <div className="video-transport">{[["Full source",10000,10000],["Center half",5000,5000]] .map(([label,width,height])=><button className="secondary-button" key={label} onClick={()=>edits(edit=>{edit.crop={x:(10000-Number(width))/2,y:(10000-Number(height))/2,width:Number(width),height:Number(height)};})}>{label}</button>)}</div>
          <div className="video-transport">{[["Square crop",1],["Landscape crop",16/9],["Portrait crop",9/16]].map(([label,aspect])=><button className="secondary-button" key={label} onClick={()=>edits(edit=>{edit.crop=cropForAspect(project.asset.info,Number(aspect));})}>{label}</button>)}</div>
        </fieldset>
        <div className="video-transport">
          <button className="secondary-button" disabled={busy} onClick={()=>void action(async()=>{await flush();const destination=await save({defaultPath:`${project.title}.openphoto-video.json`,filters:[{name:"Video project",extensions:["json"]}]});if(destination){await invoke("write_video_project_copy",{project:session.history.present,destination});setMessage("Project copy saved. The video source is referenced, not copied.");}})}>Save project copy…</button>
          <button className="secondary-button" disabled={busy} onClick={()=>void action(async()=>{await flush();const file=await open({multiple:false,filters:[{name:"Original project video",extensions:["mp4","mov","m4v","mkv","webm","avi"]}]});if(typeof file==="string")adopt(await invoke<LoadedProject>("relink_video_project",{root,id:project.id,path:file,expectedRevision:session.revision}));})}>Relink original video…</button>
        </div>
        <fieldset><legend>Export MP4</legend>
          <label>Output size<select aria-label="Video export size" value={exportSize} onChange={event=>setExportSize(event.target.value as ExportSize)}><option value="project">Project canvas ({project.output.width} × {project.output.height})</option><option value="hd1080">Up to 1080p (keep smaller canvases)</option></select></label>
          <p>Saved trim: {clockTime(markSeconds(c.source_in))} to {clockTime(markSeconds(c.source_out))}. H.264 SDR video; {e.muted?"audio removed":project.asset.info.audio.length?"first audio track as AAC stereo, with saved volume":"no source audio"}. Existing files are never overwritten. Subtitles, chapters, and source metadata are not copied.</p>
          {!e.muted&&project.asset.info.audio.length>1&&<label><input aria-label="Confirm first audio track only" type="checkbox" checked={acknowledgeAudio} onChange={event=>setAcknowledgeAudio(event.target.checked)}/>Use only the first audio track; exclude the others.</label>}
          <button className="primary-button" disabled={busy||!!exportStatus?.active||!loaded?.source_path||!!project.asset.info.hdr||(!e.muted&&project.asset.info.audio.length>1&&!acknowledgeAudio)} onClick={()=>void action(exportVideo)}>Export video MP4…</button>
          {exportStatus?.active&&<small>An export is already running. Progress and cancellation are available in the export panel.</small>}
        </fieldset>
        <small>Edits autosave locally and leave the original unchanged. Open a recent draft to resume after restart. Project files reference video relative to the selected library; relink moved originals using the same file contents. Export writes a separate validated video.</small>
      </>}
      {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
    </section>
  </div>;
}
