import {useCallback,useEffect,useLayoutEffect,useRef,useState,type Dispatch,type SetStateAction,type RefObject} from "react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {save} from "@tauri-apps/plugin-dialog";
import {type LoadedSequence,type SequenceSession} from "./videoSequence";
import {defaultOutput,sourceCanvas} from "./videoProject";
import {exportCanvas,exportFilename,startWorkspaceVideoExport,useVideoExport,type ExportSize} from "./videoExport";
import {ErrorNotice} from "./Ui";

export type ProjectPreviewState={source:string;busy:boolean;warnings:string[]};
export const emptyProjectPreview=():ProjectPreviewState=>({source:"",busy:false,warnings:[]});
const canvases=[{width:1920,height:1080,label:"1080p landscape"},{width:1080,height:1920,label:"1080p portrait"},{width:1080,height:1080,label:"Square"},{width:3840,height:2160,label:"4K landscape"},{width:2160,height:3840,label:"4K portrait"}];
/** The stage has one owner: original playback or a compiled project, never two audio players. */
export function SequenceOutput({session,loaded,disabled,host,previewState,onPreviewState,stopRef}:{session:SequenceSession;loaded:LoadedSequence;disabled:boolean;host:RefObject<HTMLElement|null>;previewState:ProjectPreviewState;onPreviewState:Dispatch<SetStateAction<ProjectPreviewState>>;stopRef:RefObject<(()=>void)|null>}){
  const project=session.history.present,used=project.clips.map(c=>({clip:c,asset:project.assets.find(a=>a.id===c.asset_id)!}));
  const ready=used.length>0&&used.every(({asset})=>loaded.sources.some(s=>s.asset_id===asset.id&&s.root&&s.path)),hdr=used.some(({asset})=>asset.info.hdr),available=ready&&!hdr;
  const [size,setSize]=useState<ExportSize>("project"),[ack,setAck]=useState(false),[error,setError]=useState(""),[exporting,setExporting]=useState(false);
  const exportOpening=useRef(false),previewOpening=useRef(false),active=useRef(true),snapshot=useRef(project),cancel=useRef(()=>{});snapshot.current=project;
  const exportStatus=useVideoExport(),needsAck=used.some(({clip,asset})=>!clip.edits.muted&&asset.info.audio.length>1);
  const ackSources=[...new Set(used.filter(({clip,asset})=>!clip.edits.muted&&asset.info.audio.length>1).map(({asset})=>asset.id))].sort().join("|");
  const stop=useCallback(()=>{cancel.current();previewOpening.current=false;onPreviewState(emptyProjectPreview());},[onPreviewState]);
  useEffect(()=>{active.current=true;return()=>{active.current=false;cancel.current();};},[]);
  useEffect(()=>setAck(false),[project.id,ackSources]);
  useLayoutEffect(()=>{stopRef.current=stop;return()=>{if(stopRef.current===stop)stopRef.current=null;};},[stop,stopRef]);
  useLayoutEffect(()=>{stop();return()=>cancel.current();},[project,stop]);
  function validateDrafts(){const invalid=host.current?.querySelector<HTMLInputElement>('input:invalid,[aria-invalid="true"]');if(invalid){invalid.focus();invalid.reportValidity();throw Error("Correct the highlighted field before rendering or exporting.");}}
  async function renderPreview(){
    if(!available||previewOpening.current)return;
    try{validateDrafts();}catch(cause){setError(String(cause));return;}
    stop();setError("");previewOpening.current=true;
    let valid=true,id="";let timer:ReturnType<typeof setTimeout>|undefined;
    cancel.current=()=>{valid=false;previewOpening.current=false;clearTimeout(timer);if(id)void invoke("cancel_video_preview",{id}).catch(()=>{});};
    onPreviewState({source:"",busy:true,warnings:[]});
    function fail(cause:unknown){if(valid&&active.current){cancel.current();onPreviewState(emptyProjectPreview());setError(String(cause));}}
    try{
      await session.flush();if(!valid||!active.current||snapshot.current!==project)return;
      id=await invoke<string>("start_workspace_video_preview",{workspaceId:session.workspaceId,id:project.id,expectedRevision:session.revision});
      if(!valid||!active.current||snapshot.current!==project){void invoke("cancel_video_preview",{id}).catch(()=>{});return;}
      const poll=async()=>{try{
        const status=await invoke<{state:string;error:string|null;result:{source:string;warnings:string[]}|null}>("video_preview_status",{id});
        if(!valid||!active.current||snapshot.current!==project)return;
        if(status.state==="failed")throw Error(status.error||"Project preview failed");
        if(status.state==="complete"&&status.result){previewOpening.current=false;onPreviewState({source:convertFileSrc(status.result.source),busy:false,warnings:status.result.warnings});return;}
        timer=setTimeout(()=>void poll(),400);
      }catch(cause){fail(cause);}};await poll();
    }catch(cause){fail(cause);}
  }
  async function exportProject(){
    if(!available||exportOpening.current)return;exportOpening.current=true;setExporting(true);
    try{
      validateDrafts();setError("");await session.flush();if(!active.current||snapshot.current!==project)return;
      const destination=await save({defaultPath:exportFilename(project.title),filters:[{name:"MP4 video",extensions:["mp4"]}]});
      if(!destination||!active.current)return;if(snapshot.current!==project)throw Error("The project changed while choosing an output file. Export again to use the current edits.");
      await session.flush();if(!active.current||snapshot.current!==project)return;
      await startWorkspaceVideoExport(session.workspaceId!,project.id,session.revision,project.title,{destination,size,acknowledge_first_audio:ack});
    }catch(cause){if(active.current)setError(String(cause));}finally{exportOpening.current=false;if(active.current)setExporting(false);}
  }
  const first=used[0]?.asset||project.assets[0],sourceSize=sourceCanvas(first.info);
  const exported=exportCanvas(project.output,size);
  const options=sourceSize&&!canvases.some(c=>c.width===sourceSize.width&&c.height===sourceSize.height)?[{...sourceSize,label:"First clip source resolution"},...canvases]:canvases;
  return <section className="sequence-output" aria-label="Project output"><h2>Canvas, preview and export</h2>
    <fieldset disabled={disabled}><div className="video-edit-fields"><label>Project canvas<select aria-label="Video canvas" value={`${project.output.width}x${project.output.height}`} onChange={e=>{const[width,height]=e.target.value.split("x").map(Number);session.commit({...project,output:{...project.output,width,height}});}}>{options.map(c=><option key={c.label} value={`${c.width}x${c.height}`}>{c.label} ({c.width} × {c.height})</option>)}{!options.some(c=>c.width===project.output.width&&c.height===project.output.height)&&<option value={`${project.output.width}x${project.output.height}`}>Saved canvas ({project.output.width} × {project.output.height})</option>}</select></label>
    <label>Canvas placement<select aria-label="Video canvas placement" value={project.output.fit} onChange={e=>session.commit({...project,output:{...project.output,fit:e.target.value as "fit"|"fill"}})}><option value="fit">Fit, with black borders</option><option value="fill">Fill, with centered crop</option></select></label>
    <label>Joined clips frame rate<select aria-label="Sequence frame rate" disabled={project.clips.length<2} value={project.frame_rate} onChange={e=>session.commit({...project,frame_rate:Number(e.target.value) as 30|60})}><option value={30}>30 fps</option><option value={60}>60 fps</option></select></label>
    <label>Output size<select aria-label="Video export size" value={size} onChange={e=>setSize(e.target.value as ExportSize)}><option value="project">Project resolution</option><option value="hd1080">Up to 1080p</option><option value="uhd4k">Up to 4K</option></select></label></div>
    <button className="secondary-button" onClick={()=>session.commit({...project,output:defaultOutput(first.info)})}>Reset project canvas</button>
    {needsAck&&<label><input aria-label="Confirm first audio track only" type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)}/>Use only each video's first audio track as AAC stereo; exclude its other tracks.</label>}
    <div className="video-transport"><button className="secondary-button" disabled={!available||previewState.busy} onClick={()=>void renderPreview()}>Preview project edits</button><button className="primary-button" disabled={!available||exporting||!!exportStatus?.active||(needsAck&&!ack)} onClick={()=>void exportProject()}>Export project MP4…</button>{previewState.busy&&<button className="secondary-button" onClick={stop}>Cancel preview</button>}</div></fieldset>
    {error&&<ErrorNotice title="The project output could not finish" error={error}/>}
    {hdr&&<p>HDR output needs tested tone mapping. Original playback remains available.</p>}
    {!ready&&!!used.length&&<p>Relink missing timeline sources before previewing or exporting the project.</p>}
    <p>One continuous MP4/H.264 SDR video with saved edits. Originals stay unchanged and existing output files are never overwritten. A larger canvas does not add source detail.</p>
    <p>Export resolution: {exported.width} × {exported.height}. Resolution presets keep the project's shape and do not enlarge a smaller canvas.</p>
    <p>{project.clips.length===1?"One clip keeps its source frame timing.":`Joined clips use ${project.frame_rate} fps with linked original audio and silence for muted or silent intervals.`}</p>
  </section>;
}
