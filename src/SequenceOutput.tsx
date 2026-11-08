import {useCallback,useEffect,useId,useLayoutEffect,useRef,useState,type Dispatch,type SetStateAction,type RefObject,type ReactNode} from "react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {save} from "@tauri-apps/plugin-dialog";
import {type LoadedSequence,type SequenceSession} from "./videoSequence";
import {defaultOutput,sourceCanvas} from "./videoProject";
import {exportCanvas,exportFilename,startWorkspaceVideoExport,startWorkspaceTransitionExport,useVideoExport,type ExportSize} from "./videoExport";
import {transitionRenderRequest,type TransitionState,type LayerPlan} from "./workspaceTransitions";
import {assetGeometry,usesSequenceFrameClock,videoInfo} from "./sequenceMedia";
import {ErrorNotice} from "./Ui";
import {socialCanvases} from "./socialCanvases";

export type ProjectPreviewState={source:string;busy:boolean;warnings:string[];kind?:"project"|"transitions";clock?:LayerPlan};
export const emptyProjectPreview=():ProjectPreviewState=>({source:"",busy:false,warnings:[]});
export type SequenceOutputParts={settings:ReactNode;previewControls:ReactNode;exportControl:ReactNode;notices:ReactNode;feedback:ReactNode;previewCrossfades:(state:TransitionState)=>Promise<void>;exportCrossfades:(state:TransitionState)=>Promise<void>;canPreviewCrossfades:boolean;canExportCrossfades:boolean};
const canvases=socialCanvases;
/** The stage has one owner: original playback or a compiled project, never two audio players. */
export function SequenceOutput({session,loaded,disabled,host,previewState,onPreviewState,stopRef,children,onInvalidDraft,proposalPresent=false}:{session:SequenceSession;loaded:LoadedSequence;disabled:boolean;host:RefObject<HTMLElement|null>;previewState:ProjectPreviewState;onPreviewState:Dispatch<SetStateAction<ProjectPreviewState>>;stopRef:RefObject<(()=>void)|null>;children?:(parts:SequenceOutputParts)=>ReactNode;onInvalidDraft?:(input:HTMLInputElement)=>void;proposalPresent?:boolean}){
  const project=session.history.present,used=project.clips.map(c=>({clip:c,asset:project.assets.find(a=>a.id===c.asset_id)!}));
  const ready=used.length>0&&used.every(({asset})=>loaded.sources.some(s=>s.asset_id===asset.id&&s.root&&s.path)),hdr=used.some(({asset})=>videoInfo(asset)?.hdr),available=ready&&!hdr;
  const [size,setSize]=useState<ExportSize>("project"),[ack,setAck]=useState(false),[error,setError]=useState(""),[exporting,setExporting]=useState(false);
  const exportOpening=useRef(false),previewOpening=useRef(false),active=useRef(true),snapshot=useRef(project),cancel=useRef(()=>{});snapshot.current=project;
  const activeTransition=useRef<TransitionState|null>(null),proposalView=previewState.kind==="transitions"&&(previewState.busy||!!previewState.source);
  const exportStatus=useVideoExport(),needsAck=used.some(({clip,asset})=>!clip.edits.muted&&(videoInfo(asset)?.audio.length||0)>1);
  const audioChoiceId=useId();
  const ackSources=[...new Set(used.filter(({clip,asset})=>!clip.edits.muted&&(videoInfo(asset)?.audio.length||0)>1).map(({asset})=>asset.id))].sort().join("|");
  const stop=useCallback(()=>{cancel.current();previewOpening.current=false;activeTransition.current=null;onPreviewState(emptyProjectPreview());},[onPreviewState]);
  useEffect(()=>{active.current=true;return()=>{active.current=false;cancel.current();};},[]);
  useEffect(()=>setAck(false),[project.id,ackSources]);
  useLayoutEffect(()=>{stopRef.current=stop;return()=>{if(stopRef.current===stop)stopRef.current=null;};},[stop,stopRef]);
  useLayoutEffect(()=>{stop();return()=>cancel.current();},[project,stop]);
  function validateDrafts(){const invalid=host.current?.querySelector<HTMLInputElement>('input:invalid,[aria-invalid="true"]');if(invalid){onInvalidDraft?.(invalid);requestAnimationFrame(()=>{for(let parent=invalid.parentElement;parent;parent=parent.parentElement){if(parent instanceof HTMLDetailsElement)parent.open=true;}invalid.focus();invalid.reportValidity();});throw Error("Correct the highlighted field before rendering or exporting.");}}
  async function renderPreview(transition?:TransitionState){
    if(!available||previewOpening.current)return;
    try{validateDrafts();}catch(cause){setError(String(cause));return;}
    stop();setError("");previewOpening.current=true;
    activeTransition.current=transition||null;
    const scope={kind:transition?"transitions" as const:"project" as const,clock:transition?.timing||undefined};
    let valid=true,id="";let timer:ReturnType<typeof setTimeout>|undefined;
    cancel.current=()=>{valid=false;previewOpening.current=false;clearTimeout(timer);if(id)void invoke("cancel_video_preview",{id}).catch(()=>{});};
    onPreviewState({source:"",busy:true,warnings:[],...scope});
    function fail(cause:unknown){if(valid&&active.current){cancel.current();activeTransition.current=null;onPreviewState(emptyProjectPreview());setError(String(cause));}}
    try{
      await session.flush();if(!valid||!active.current||snapshot.current!==project)return;
      id=transition?await invoke<string>("start_workspace_transition_preview",{request:transitionRenderRequest(session,transition)}):await invoke<string>("start_workspace_video_preview",{workspaceId:session.workspaceId,id:project.id,expectedRevision:session.revision});
      if(!valid||!active.current||snapshot.current!==project){void invoke("cancel_video_preview",{id}).catch(()=>{});return;}
      const poll=async()=>{try{
        const status=await invoke<{state:string;error:string|null;result:{source:string;warnings:string[]}|null}>("video_preview_status",{id});
        if(!valid||!active.current||snapshot.current!==project)return;
        if(status.state==="failed")throw Error(status.error||"Project preview failed");
        if(status.state==="complete"&&status.result){previewOpening.current=false;onPreviewState({source:convertFileSrc(status.result.source),busy:false,warnings:status.result.warnings,...scope});return;}
        timer=setTimeout(()=>void poll(),400);
      }catch(cause){fail(cause);}};await poll();
    }catch(cause){fail(cause);}
  }
  async function exportProject(transition:TransitionState|undefined=activeTransition.current||undefined){
    if(!available||exportOpening.current)return;exportOpening.current=true;setExporting(true);
    try{
      validateDrafts();setError("");await session.flush();if(!active.current||snapshot.current!==project)return;
      const destination=await save({defaultPath:transition?exportFilename(project.title).replace(/\.edited\.mp4$/,".crossfades.mp4"):exportFilename(project.title),filters:[{name:"MP4 video",extensions:["mp4"]}]});
      if(!destination||!active.current)return;if(snapshot.current!==project)throw Error("The project changed while choosing an output file. Export again to use the current edits.");
      await session.flush();if(!active.current||snapshot.current!==project)return;
      const options={destination,size,acknowledge_first_audio:ack};
      if(transition)await startWorkspaceTransitionExport(transitionRenderRequest(session,transition),`${project.title} — crossfades`,options);
      else await startWorkspaceVideoExport(session.workspaceId!,project.id,session.revision,project.title,options);
    }catch(cause){if(active.current)setError(String(cause));}finally{exportOpening.current=false;if(active.current)setExporting(false);}
  }
  const first=used[0]?.asset||project.assets[0],sourceSize=sourceCanvas(assetGeometry(first));
  const exported=exportCanvas(project.output,size);
  const options=sourceSize&&!canvases.some(c=>c.width===sourceSize.width&&c.height===sourceSize.height)?[{...sourceSize,label:"First clip source resolution"},...canvases]:canvases;
  const settings=<section className="sequence-output" aria-label="Project output"><h2>Canvas & export settings</h2>
    <fieldset disabled={disabled}><div className="video-edit-fields"><label>Project canvas<select aria-label="Video canvas" disabled={proposalView} value={`${project.output.width}x${project.output.height}`} onChange={e=>{const[width,height]=e.target.value.split("x").map(Number);session.commit({...project,output:{...project.output,width,height}});}}>{options.map(c=><option key={c.label} value={`${c.width}x${c.height}`}>{c.label} ({c.width} × {c.height})</option>)}{!options.some(c=>c.width===project.output.width&&c.height===project.output.height)&&<option value={`${project.output.width}x${project.output.height}`}>Saved canvas ({project.output.width} × {project.output.height})</option>}</select></label>
    <label>Canvas placement<select aria-label="Video canvas placement" disabled={proposalView} value={project.output.fit} onChange={e=>session.commit({...project,output:{...project.output,fit:e.target.value as "fit"|"fill"}})}><option value="fit">Fit, with black borders</option><option value="fill">Fill, with centered crop</option></select></label>
    <label>Project frame rate<select aria-label="Sequence frame rate" disabled={proposalView||!usesSequenceFrameClock(project)} value={project.frame_rate} onChange={e=>session.commit({...project,frame_rate:Number(e.target.value) as 30|60})}><option value={30}>30 fps</option><option value={60}>60 fps</option></select></label>
    <label>Output size<select aria-label="Video export size" value={size} onChange={e=>setSize(e.target.value as ExportSize)}><option value="project">Project resolution</option><option value="hd1080">Up to 1080p</option><option value="uhd4k">Up to 4K</option></select></label></div>
    <button className="secondary-button" disabled={proposalView} onClick={()=>session.commit({...project,output:defaultOutput(assetGeometry(first))})}>Reset project canvas</button>
    {needsAck&&<label><input aria-label="Confirm first audio track only" type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)}/>Use only each video's first audio track as AAC stereo; exclude its other tracks.</label>}
    </fieldset></section>;
  const canPreviewCrossfades=!disabled&&available&&!previewState.busy,canExportCrossfades=!disabled&&available&&!exporting&&!exportStatus?.active&&!(needsAck&&!ack);
  const previewControls=<div className="video-transport"><button className="secondary-button" disabled={!canPreviewCrossfades} onClick={()=>void renderPreview()}>{proposalPresent?"Preview source timeline":"Preview project edits"}</button>{previewState.busy&&<button className="secondary-button" onClick={stop}>Cancel preview</button>}</div>;
  const exportLabel=proposalView?"Export crossfade proposal MP4…":proposalPresent?"Export source timeline MP4…":"Export project MP4…";
  const exportControl=<button className="primary-button" disabled={!canExportCrossfades} onClick={()=>void exportProject()} aria-label={exportLabel} aria-describedby={needsAck&&!ack?audioChoiceId:undefined} title={needsAck&&!ack?"Confirm the audio-track choice in Canvas & export settings":exportLabel}>{exporting?"Preparing…":proposalView?"Export crossfades":proposalPresent?"Export timeline":"Export"}</button>;
  const feedback=<>
    {error&&<ErrorNotice title="The project output could not finish" error={error}/>}
    {hdr&&<p>HDR output needs tested tone mapping. Original playback remains available.</p>}
    {!ready&&!!used.length&&<p>Relink missing timeline sources before previewing or exporting the project.</p>}
    {needsAck&&!ack&&<p id={audioChoiceId}>Before exporting, confirm the first audio track under Canvas & export settings in the clip controls.</p>}
  </>;
  const notices=<>
    <p>One continuous MP4/H.264 SDR video with saved edits. Originals stay unchanged and existing output files are never overwritten. A larger canvas does not add source detail.</p>
    <p>Export resolution: {exported.width} × {exported.height}. Resolution presets keep the project's shape and do not enlarge a smaller canvas.</p>
    <p>{!usesSequenceFrameClock(project)?"One video clip keeps its source frame timing.":`The rendered project uses ${project.frame_rate} fps with linked original audio and silence for muted or silent intervals.`}</p>
  </>;
  return children?children({settings,previewControls,exportControl,notices,feedback,previewCrossfades:renderPreview,exportCrossfades:exportProject,canPreviewCrossfades,canExportCrossfades}):<>{settings}{previewControls}{exportControl}{feedback}{notices}</>;
}
