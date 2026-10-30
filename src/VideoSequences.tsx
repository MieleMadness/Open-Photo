import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {ErrorNotice} from "./Ui";
import {VideoPlayer} from "./VideoPlayer";
import {SequenceSession,workspaceProject,workspaceProjects,type LoadedSequence,type SequenceProject} from "./videoSequence";
import {exportFilename} from "./videoExport";
import {PanelLeft,PanelRight,ArrowLeft,Plus,Search,X} from "lucide-react";
import {EditorProjectMenu} from "./EditorProjectMenu";
import {EditorPoster} from "./EditorPoster";
import {SequenceTimeline,SequenceClipInspector} from "./SequenceTimeline";
import {SequenceOutput,emptyProjectPreview} from "./SequenceOutput";
import {editTimeline} from "./timelineCommands";
import {markSeconds} from "./videoTime";
import {useVideoElement} from "./mediaPlayback";

const videoFilters=[{name:"Videos",extensions:["mp4","mov","m4v","mkv","webm","avi"]}];
type PreviousSave={id:number;revision:number|null;saved_at:number|null;title:string;clips:number;duration_seconds:number};
export function VideoSequences({roots,workspaceId,initial,onBack}:{roots:string[];workspaceId:string;initial?:LoadedSequence|null;onBack?:()=>void}){
  return workspaceId?<SequenceWorkspace key={workspaceId} roots={roots} workspaceId={workspaceId} initial={initial||undefined} onBack={onBack}/>:<p>Choose a workspace to open its editing projects.</p>;
}
function SequenceWorkspace({roots,workspaceId,initial,onBack}:{roots:string[];workspaceId:string;initial?:LoadedSequence;onBack?:()=>void}){
  const root=roots[0]||"";
  const host=useRef<HTMLElement>(null),previewStage=useRef<HTMLElement>(null),stopPreview=useRef<(()=>void)|null>(null);
  const [projectVideo,bindProjectVideo]=useVideoElement();
  const [previewState,setPreviewState]=useState(emptyProjectPreview),[playhead,setPlayhead]=useState(0),[playbackError,setPlaybackError]=useState("");
  const [session,setSession]=useState<SequenceSession|null>(null),[loaded,setLoaded]=useState<LoadedSequence|null>(null);
  const owner=useRef<SequenceSession|null>(null),generation=useRef(0),working=useRef(false);
  const [recent,setRecent]=useState<{id:string;title:string}[]>([]),[selected,setSelected]=useState(""),[selectedClip,setSelectedClip]=useState("");
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[message,setMessage]=useState("");const [,refresh]=useState(0);
  const [previous,setPrevious]=useState<{snapshot:SequenceProject;revision:number;entries:PreviousSave[]}|null>(null),[previousId,setPreviousId]=useState("");
  const [mediaOpen,setMediaOpen]=useState(true),[inspectorOpen,setInspectorOpen]=useState(()=>typeof matchMedia==="undefined"||matchMedia("(min-width:1100px)").matches),[mediaQuery,setMediaQuery]=useState("");
  const project=session?.history.present;
  useEffect(()=>setPlaybackError(""),[previewState.source]);
  useEffect(()=>{if(previewState.busy){previewStage.current?.scrollIntoView?.({block:"start"});previewStage.current?.focus({preventScroll:true});}},[previewState.busy]);
  const clip=project?.clips.find(c=>c.id===selectedClip)||project?.clips[0];
  useEffect(()=>{const token=++generation.current;void workspaceProjects(workspaceId).then(rows=>{if(token===generation.current){setRecent(rows.projects);if(rows.warnings.length)setError(rows.warnings.join("\n"));}}).catch(e=>{if(token===generation.current)setError(String(e));});return()=>{generation.current++;owner.current?.release();};},[workspaceId]);
  useEffect(()=>session?.subscribe(()=>refresh(n=>n+1)),[session]);
  useEffect(()=>{if(initial){const token=generation.current;adopt(initial,()=>token===generation.current);}},[initial,workspaceId]);
  async function run(task:(current:()=>boolean)=>Promise<void>,flush=true){
    if(working.current)return;working.current=true;setBusy(true);setError("");setMessage("");const token=generation.current,current=()=>token===generation.current;
    try{if(flush)await owner.current?.flush();if(current())await task(current);}catch(e){if(current())setError(String(e));}finally{working.current=false;if(current())setBusy(false);}
  }
  function adopt(value:LoadedSequence,current:()=>boolean){
    if(!current())return;owner.current?.release();const next=new SequenceSession(root,value,workspaceId);owner.current=next;setSession(next);setLoaded(value);setPrevious(null);setPreviousId("");setPlayhead(0);
    const nextClip=value.project.clips.find(c=>c.id===selectedClip)||value.project.clips[0];
    setSelected(nextClip?.asset_id||value.project.assets[0].id);setSelectedClip(nextClip?.id||"");
    void workspaceProjects(workspaceId).then(rows=>{if(current()){setRecent(rows.projects);if(rows.warnings.length)setError(rows.warnings.join("\n"));}}).catch(e=>{if(current())setError(String(e));});
  }
  async function chooseVideos(current:()=>boolean){const files=await open({multiple:true,defaultPath:root,filters:videoFilters});return current()?(Array.isArray(files)?files:typeof files==="string"?[files]:[]):[];}
  function seekProject(seconds:number){setPlayhead(seconds);if(projectVideo.current){const duration=projectVideo.current.duration;projectVideo.current.currentTime=Math.min(seconds,Number.isFinite(duration)?duration:seconds);}}
  function chooseBin(id:string){stopPreview.current?.();setSelected(id);}
  function selectClip(id:string){setSelectedClip(id);const clip=owner.current?.history.present.clips.find(c=>c.id===id);if(clip)setSelected(clip.asset_id);}
  const asset=project?.assets.find(a=>a.id===selected),source=loaded?.sources.find(s=>s.asset_id===selected);
  return <section ref={host} aria-label="Video editing project" className="sequence-workspace" data-ready={!!project}>
    {project&&session&&loaded?<SequenceOutput key={`output:${project.id}`} session={session} loaded={loaded} disabled={busy} host={host} previewState={previewState} onPreviewState={setPreviewState} stopRef={stopPreview} onInvalidDraft={()=>setInspectorOpen(true)}>{({settings,previewControls,exportControl,notices:outputNotices,feedback:outputFeedback})=><>
      <header className="editor-project-bar"><h1 className="visually-hidden">Video editor</h1>
        {onBack&&<button className="icon-button" onClick={onBack} aria-label="Back to Library" title="Back to Library"><ArrowLeft size={18}/></button>}
        <div className="editor-project-identity"><span className="eyebrow">CREATE & EDIT</span><label>Project name<input type="text" aria-label="Video project name" disabled={busy} maxLength={200} value={project.title} onChange={e=>{try{session.commit({...project,title:e.target.value});setError("");}catch(cause){setError(String(cause));}}}/></label>
        <p className="results-note" role="status">{session.status==="saved"?"All project changes saved locally.":session.status==="saving"?"Saving project changes…":"Project changes need attention."}</p></div>
        <EditorProjectMenu key={project.id}>    <details className="project-file-actions" open={!project}><summary>{project?"Switch or create project":"Open a video project"}</summary><div className="video-transport">
      <button className="primary-button" disabled={busy||!roots.length} onClick={()=>void run(async current=>{const paths=await chooseVideos(current);if(paths.length)adopt(await workspaceProject(workspaceId,{type:"create",paths}),current);})}>New video project…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const path=await open({multiple:false,filters:[{name:"Open-Photo video project",extensions:["json"]}]});if(typeof path==="string"&&current()){adopt(await workspaceProject(workspaceId,{type:"import",path}),current);if(current())setMessage("Opened a new project copy. Existing drafts remain unchanged; this copy belongs to the current workspace.");}})}>Open project file…</button>
      {!!recent.length&&<label>Saved projects<select aria-label="Saved editing projects" disabled={busy} value="" onChange={e=>{const id=e.target.value;if(id)void run(async current=>adopt(await workspaceProject(workspaceId,{type:"load",id}),current));}}><option value="">Choose a project…</option>{recent.map(p=><option key={p.id} value={p.id}>{p.title}</option>)}</select></label>}
    </div></details><div className="video-transport"><button className="secondary-button" onClick={()=>void run(async current=>{const destination=await save({defaultPath:exportFilename(project.title).replace(/\.edited\.mp4$/,".openphoto-sequence.json"),filters:[{name:"Video project",extensions:["json"]}]});if(destination&&current()){await invoke("write_video_sequence_copy",{project:owner.current!.history.present,destination});if(current())setMessage("Project copy saved. Source videos are referenced and remain unchanged.");}})}>Save project copy…</button>
        </div>

      <details className="project-file-actions"><summary>Previous saves and recovery</summary>
        <p>Recover a previous save as a new project copy. Your current project is kept.</p>
        <div className="video-transport"><button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const entries=await invoke<PreviousSave[]>("workspace_video_history",{workspaceId,id:project.id});if(current()){setPrevious({snapshot:owner.current!.history.present,revision:owner.current!.revision,entries});setPreviousId("");}})}>Refresh previous saves</button>
        {previous&&<><label>Previous save<select aria-label="Previous timeline save" disabled={busy} value={previousId} onChange={e=>setPreviousId(e.target.value)}><option value="">Choose a save…</option>{previous.entries.map(item=><option key={item.id} value={item.id}>{item.revision===null?"Earlier save":`Revision ${item.revision}`} · {item.title} · {item.clips} clips · {item.saved_at===null?"Save time unavailable":new Date(item.saved_at*1000).toLocaleString()}</option>)}</select></label><button className="secondary-button" disabled={busy||!previousId||previous.snapshot!==project||previous.revision!==session.revision} onClick={()=>void run(async current=>{const value=await workspaceProject(workspaceId,{type:"recover",id:project.id,history_id:Number(previousId),expected_revision:previous.revision});adopt(value,current);if(current())setMessage("Recovered as a new project copy. The current project remains saved.");})}>Recover timeline copy</button></>}
        </div>
        {previous&&!previous.entries.length&&<p>No previous saves yet. Changes are retained automatically, up to 20 saves.</p>}
        {previous&&(previous.snapshot!==project||previous.revision!==session.revision)&&<p>Refresh previous saves after editing before recovering a copy.</p>}
      </details></EditorProjectMenu>
        <div className="editor-panel-toggles"><button className="icon-button" aria-label="Show media panel" title="Show media panel" aria-pressed={mediaOpen} onClick={()=>setMediaOpen(v=>!v)}><PanelLeft size={18}/></button><button className="icon-button" aria-label="Show clip controls" title="Show clip controls" aria-pressed={inspectorOpen} onClick={()=>setInspectorOpen(v=>!v)}><PanelRight size={18}/></button></div>
        {exportControl}
      </header>
      <div className="editor-feedback">{outputFeedback}
    {busy&&<p className="results-note" role="status">Verifying videos and saving the project…</p>}
    {(error||session?.error)&&<ErrorNotice title="The project needs attention" error={error||session?.error}>{session?.error&&<>
      <button className="secondary-button" disabled={busy} onClick={()=>{setError("");session.retry();}}>Retry project save</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const destination=await save({defaultPath:"unsaved-video-sequence.json"});if(destination&&current()){await invoke("write_video_sequence_copy",{project:session.history.present,destination});if(current())setMessage("Unsaved project copy saved.");}},false)}>Save unsaved project copy…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const value=await workspaceProject(workspaceId,{type:"load",id:session.history.present.id});if(current()){session.discard();adopt(value,current);}},false)}>Discard unsaved changes and reopen draft</button>
    </>}</ErrorNotice>}
    {message&&<p className="results-note" role="status">{message}</p>}</div>
      <div className="editor-panels" data-media={mediaOpen} data-inspector={inspectorOpen}>
        <section aria-label="Video media bin" className="sequence-bin" hidden={!mediaOpen}>
          <div className="editor-panel-heading"><h2>Media <small>{project.assets.length} / 50 videos</small></h2></div>
          <button className="secondary-button editor-import" disabled={busy||!roots.length} onClick={()=>void run(async current=>{const paths=await chooseVideos(current);if(paths.length){const before=project.assets.length;const value=await workspaceProject(workspaceId,{type:"add",id:project.id,paths,expected_revision:session.revision});adopt(value,current);if(current())setMessage(`${value.project.assets.length-before} videos added. Identical videos already in the bin were skipped.`);}})}><Plus size={16}/>Add videos…</button>
          <label className="editor-media-search"><Search size={16}/><input aria-label="Search project media" placeholder="Search media" value={mediaQuery} onChange={e=>setMediaQuery(e.target.value)}/></label>
          <ul>{project.assets.filter(a=>a.relative_path.toLocaleLowerCase().includes(mediaQuery.trim().toLocaleLowerCase())).map(a=>{const status=loaded.sources.find(s=>s.asset_id===a.id);return <li key={a.id}><button className={selected===a.id?"sequence-asset selected":"sequence-asset"} aria-pressed={selected===a.id} disabled={busy} onClick={()=>chooseBin(a.id)} title={a.relative_path}><EditorPoster root={status?.root||undefined} path={status?.path||undefined} identity={a.sha256}/><strong>{a.relative_path.split("/").at(-1)}</strong><small>{a.info.duration_seconds.toFixed(2)} s · {a.info.width} × {a.info.height}{!status?.path?" · Needs relinking":""}</small></button></li>;})}</ul>
          {mediaQuery&&!project.assets.some(a=>a.relative_path.toLocaleLowerCase().includes(mediaQuery.trim().toLocaleLowerCase()))&&<p>No matching media. Try another name.</p>}
          <details className="editor-source-actions"><summary>Selected source options</summary>
        {asset&&<>
          <div className="video-transport"><button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const path=await open({multiple:false,defaultPath:root,filters:videoFilters});if(typeof path==="string"&&current())adopt(await workspaceProject(workspaceId,{type:"relink",id:project.id,asset_id:asset.id,path,expected_revision:session.revision}),current);})}>Relink original video…</button>
            <button className="secondary-button" disabled={busy||project.assets.length===1} onClick={()=>void run(async current=>adopt(await workspaceProject(workspaceId,{type:"remove_asset",id:project.id,asset_id:asset.id,expected_revision:session.revision}),current))}>Remove from project</button></div><small>Removing a video from this project keeps its original file unchanged.</small>
        </>}</details>
        </section>
        <section ref={previewStage} tabIndex={-1} className="sequence-preview" aria-label="Video preview stage"><div className="editor-stage-heading"><h2>{previewState.busy||previewState.source?"Project preview":"Source preview"}</h2>{previewControls}</div><div className="editor-stage-content">
        {previewState.busy?<p role="status">Building the continuous project preview…</p>:previewState.source?<><video ref={bindProjectVideo} aria-label="Edited project preview" src={previewState.source} controls onError={event=>setPlaybackError(`The project preview could not play (media error ${event.currentTarget.error?.code||"unknown"}). Rebuild the preview or export to open it in the Windows player.`)} onLoadedMetadata={()=>{if(projectVideo.current)seekProject(Math.min(playhead,projectVideo.current.duration||0));}} onTimeUpdate={()=>{if(projectVideo.current)setPlayhead(projectVideo.current.currentTime);}}/>{playbackError&&<ErrorNotice title="The project preview could not play" error={playbackError}/>}<button className="secondary-button" onClick={()=>stopPreview.current?.()}>Show original source</button><ul>{previewState.warnings.map(w=><li key={w}>{w}</li>)}</ul></>:asset&&<><p>{asset.relative_path}</p>{source?.path?<VideoPlayer key={source.path} root={source.root||root} path={source.path} autoLoad compact selection={clip?.asset_id===selected?{start:clip.source_in,end:clip.source_out,disabled:busy,onChange:(start,end)=>{if(busy)return;try{session.commit(editTimeline(session.history.present,{type:"trim",clipId:clip.id,start:markSeconds(start),end:markSeconds(end)}));setError("");}catch(cause){setError(String(cause));}}}:undefined} onOpenExternal={()=>{void invoke("open_video",{root:source.root||root,path:source.path}).catch(e=>setError(String(e)));}}/>:<p role="alert" className="error">This video needs relinking: {source?.status}</p>}</>}</div></section>
        <aside className="editor-inspector" aria-label="Clip inspector" hidden={!inspectorOpen}><div className="editor-panel-heading"><h2>Controls</h2><button className="icon-button" aria-label="Hide clip controls" title="Hide clip controls" onClick={()=>setInspectorOpen(false)}><X size={18}/></button></div><SequenceClipInspector session={session} disabled={busy} selected={clip?.id||""} onSelected={selectClip} position={playhead}/>{settings}<details className="editor-output-notes"><summary>Output information</summary>{outputNotices}</details></aside>
      </div>
      <SequenceTimeline key={`timeline:${project.id}`} session={session} disabled={busy} assetId={selected} onSelectAsset={setSelected} selected={clip?.id||""} onSelected={selectClip} position={playhead} onPosition={seekProject}/>
    </>}</SequenceOutput>:<div className="editor-empty"><span className="eyebrow">CREATE & EDIT</span><h1>Video editor</h1><p>Import videos to arrange one continuous project.</p>{onBack&&<button className="secondary-button" onClick={onBack}>Back to Library</button>}    <details className="project-file-actions" open={!project}><summary>{project?"Switch or create project":"Open a video project"}</summary><div className="video-transport">
      <button className="primary-button" disabled={busy||!roots.length} onClick={()=>void run(async current=>{const paths=await chooseVideos(current);if(paths.length)adopt(await workspaceProject(workspaceId,{type:"create",paths}),current);})}>New video project…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const path=await open({multiple:false,filters:[{name:"Open-Photo video project",extensions:["json"]}]});if(typeof path==="string"&&current()){adopt(await workspaceProject(workspaceId,{type:"import",path}),current);if(current())setMessage("Opened a new project copy. Existing drafts remain unchanged; this copy belongs to the current workspace.");}})}>Open project file…</button>
      {!!recent.length&&<label>Saved projects<select aria-label="Saved editing projects" disabled={busy} value="" onChange={e=>{const id=e.target.value;if(id)void run(async current=>adopt(await workspaceProject(workspaceId,{type:"load",id}),current));}}><option value="">Choose a project…</option>{recent.map(p=><option key={p.id} value={p.id}>{p.title}</option>)}</select></label>}
    </div></details>
    {busy&&<p className="results-note" role="status">Verifying videos and saving the project…</p>}
    {(error||session?.error)&&<ErrorNotice title="The project needs attention" error={error||session?.error}>{session?.error&&<>
      <button className="secondary-button" disabled={busy} onClick={()=>{setError("");session.retry();}}>Retry project save</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const destination=await save({defaultPath:"unsaved-video-sequence.json"});if(destination&&current()){await invoke("write_video_sequence_copy",{project:session.history.present,destination});if(current())setMessage("Unsaved project copy saved.");}},false)}>Save unsaved project copy…</button>
      <button className="secondary-button" disabled={busy} onClick={()=>void run(async current=>{const value=await workspaceProject(workspaceId,{type:"load",id:session.history.present.id});if(current()){session.discard();adopt(value,current);}},false)}>Discard unsaved changes and reopen draft</button>
    </>}</ErrorNotice>}
    {message&&<p className="results-note" role="status">{message}</p>}</div>}
  </section>;
}
