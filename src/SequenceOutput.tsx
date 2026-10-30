import {useCallback,useEffect,useLayoutEffect,useRef,useState,type RefObject} from "react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {save} from "@tauri-apps/plugin-dialog";
import {singleRenderProject,type LoadedSequence,type SequenceSession} from "./videoSequence";
import {defaultOutput,sourceCanvas} from "./videoProject";
import {exportFilename,startWorkspaceVideoExport,useVideoExport,type ExportSize} from "./videoExport";
import {ErrorNotice} from "./Ui";

const canvases=[{width:1920,height:1080,label:"1080p landscape"},{width:1080,height:1920,label:"1080p portrait"},{width:1080,height:1080,label:"Square"},{width:3840,height:2160,label:"4K landscape"},{width:2160,height:3840,label:"4K portrait"}];
/** Output consumes the same project snapshot regardless of clip count. Export settings stay separate. */
export function SequenceOutput({session,loaded,disabled,host}:{session:SequenceSession;loaded:LoadedSequence;disabled:boolean;host:RefObject<HTMLElement|null>}){
  const project=session.history.present,single=singleRenderProject(project),source=loaded.sources.find(s=>s.asset_id===single?.asset.id);
  const [size,setSize]=useState<ExportSize>("project"),[ack,setAck]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(""),[preview,setPreview]=useState(""),[warnings,setWarnings]=useState<string[]>([]);
  const [exporting,setExporting]=useState(false),exportOpening=useRef(false);
  const active=useRef(true),snapshot=useRef(project),cancel=useRef(()=>{});snapshot.current=project;
  const exportStatus=useVideoExport(),needsAck=!!single&&!single.clips[0].edits.muted&&single.asset.info.audio.length>1;
  const stop=useCallback(()=>{cancel.current();setPreview("");setBusy(false);setWarnings([]);},[]);
  useEffect(()=>{active.current=true;return()=>{active.current=false;cancel.current();};},[]);
  useEffect(()=>setAck(false),[project.id,single?.asset.id]);
  useLayoutEffect(()=>{stop();return()=>cancel.current();},[project,stop]);
  function validateDrafts(){const invalid=host.current?.querySelector<HTMLInputElement>('input:invalid,[aria-invalid="true"]');if(invalid){invalid.focus();invalid.reportValidity();throw Error("Correct the highlighted field before rendering or exporting.");}}
  async function renderPreview(){
    if(!single||!source?.path||!source.root)return;let valid=true,id="";let timer:ReturnType<typeof setTimeout>|undefined;
    try{
      validateDrafts();await session.flush();if(!active.current||snapshot.current!==project)return;
      stop();setError("");setBusy(true);cancel.current=()=>{valid=false;clearTimeout(timer);if(id)void invoke("cancel_video_preview",{id}).catch(()=>{});};
      id=await invoke<string>("start_workspace_video_preview",{workspaceId:session.workspaceId,id:project.id,expectedRevision:session.revision});
      if(!valid||!active.current||snapshot.current!==project){void invoke("cancel_video_preview",{id}).catch(()=>{});return;}
      const poll=async()=>{try{
        const status=await invoke<{state:string;error:string|null;result:{source:string;warnings:string[]}|null}>("video_preview_status",{id});
        if(!valid||!active.current||snapshot.current!==project)return;
        if(status.state==="failed")throw Error(status.error||"Project preview failed");
        if(status.state==="complete"&&status.result){setPreview(convertFileSrc(status.result.source));setWarnings(status.result.warnings);setBusy(false);return;}
        timer=setTimeout(()=>void poll(),400);
      }catch(cause){if(valid&&active.current){cancel.current();setBusy(false);setError(String(cause));}}};await poll();
    }catch(cause){if(valid&&active.current){cancel.current();setBusy(false);setError(String(cause));}}
  }
  async function exportProject(){
    if(!single||!source?.path||!source.root||exportOpening.current)return;
    exportOpening.current=true;setExporting(true);
    try{
      validateDrafts();setError("");await session.flush();
      if(!active.current||snapshot.current!==project)return;
      const destination=await save({defaultPath:exportFilename(project.title),filters:[{name:"MP4 video",extensions:["mp4"]}]});
      if(!destination||!active.current)return;
      if(snapshot.current!==project)throw Error("The project changed while choosing an output file. Export again to use the current edits.");
      await session.flush();if(!active.current||snapshot.current!==project)return;
      await startWorkspaceVideoExport(session.workspaceId!,project.id,session.revision,project.title,{destination,size,acknowledge_first_audio:ack});
    }catch(cause){if(active.current)setError(String(cause));}finally{exportOpening.current=false;if(active.current)setExporting(false);}
  }
  const available=!!single&&!!source?.root&&!!source.path&&!single.asset.info.hdr;
  const sourceSize=single?sourceCanvas(single.asset.info):null;
  const options=sourceSize&&!canvases.some(c=>c.width===sourceSize.width&&c.height===sourceSize.height)?[{...sourceSize,label:"Source resolution"},...canvases]:canvases;
  return <section className="sequence-output" aria-label="Project output"><h2>Canvas, preview and export</h2>
    <fieldset disabled={disabled}><div className="video-edit-fields"><label>Project canvas<select aria-label="Video canvas" value={`${project.output.width}x${project.output.height}`} onChange={e=>{const[width,height]=e.target.value.split("x").map(Number);session.commit({...project,output:{...project.output,width,height}});}}>{options.map(c=><option key={c.label} value={`${c.width}x${c.height}`}>{c.label} ({c.width} × {c.height})</option>)}{!options.some(c=>c.width===project.output.width&&c.height===project.output.height)&&<option value={`${project.output.width}x${project.output.height}`}>Saved canvas ({project.output.width} × {project.output.height})</option>}</select></label>
    <label>Canvas placement<select aria-label="Video canvas placement" value={project.output.fit} onChange={e=>session.commit({...project,output:{...project.output,fit:e.target.value as "fit"|"fill"}})}><option value="fit">Fit, with black borders</option><option value="fill">Fill, with centered crop</option></select></label>
    <label>Output size<select aria-label="Video export size" value={size} onChange={e=>setSize(e.target.value as ExportSize)}><option value="project">Project resolution</option><option value="hd1080">Up to 1080p</option></select></label></div>
    <button className="secondary-button" onClick={()=>session.commit({...project,output:defaultOutput(project.assets[0].info)})}>Reset project canvas</button>
    {needsAck&&<label><input aria-label="Confirm first audio track only" type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)}/>Use only the first audio track as AAC stereo; exclude the others.</label>}
    <div className="video-transport"><button className="secondary-button" disabled={!available||busy} onClick={()=>void renderPreview()}>Preview project edits</button><button className="primary-button" disabled={!available||exporting||!!exportStatus?.active||(needsAck&&!ack)} onClick={()=>void exportProject()}>Export project MP4…</button>{(busy||preview)&&<button className="secondary-button" onClick={stop}>{busy?"Cancel preview":"Close edited preview"}</button>}</div></fieldset>
    {busy&&<p role="status">Rendering the saved project…</p>}{preview&&<><p>Edited project preview</p><video aria-label="Edited project preview" src={preview} controls/><ul>{warnings.map(w=><li key={w}>{w}</li>)}</ul></>}
    {error&&<ErrorNotice title="The project output could not finish" error={error}/>}
    {!single&&<p>Assembled preview and export for multiple timeline clips are the next rendering milestone.</p>}
    {single?.asset.info.hdr&&<p>HDR output needs tested tone mapping. Original playback remains available.</p>}
    <p>MP4/H.264 SDR with saved edits. Original media stays unchanged; export creates a separate validated file and never overwrites an existing file. A larger canvas does not add source detail.</p>
  </section>;
}
