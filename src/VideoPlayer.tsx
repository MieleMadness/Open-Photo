import {useEffect,useRef,useState,type KeyboardEvent} from "react";
import {FastForward,Rewind} from "lucide-react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {ErrorNotice} from "./Ui";
import {VideoDetails,type VideoInfo} from "./VideoDetails";
import {clockTime,markSeconds,markTime,type VideoTime} from "./videoTime";

type Assets={source:string;proxy:boolean;info:VideoInfo;filmstrip:{seconds:number;image:string}[];warnings:string[]};
type Status={state:"queued"|"running"|"complete"|"failed";result:Assets|null;error:string|null};

export type VideoSelection={start:VideoTime;end:VideoTime;onChange:(start:VideoTime,end:VideoTime)=>void};
export function VideoPlayer({root,path,onOpenExternal,selection,autoLoad=false}:{root:string;path:string;onOpenExternal:()=>void;selection?:VideoSelection;autoLoad?:boolean}) {
  const video=useRef<HTMLVideoElement>(null);
  const current=useRef(0);
  const restore=useRef(0);
  const rangePlaying=useRef(false);
  const cancelJob=useRef(()=>{});
  const [requested,setRequested]=useState(autoLoad);
  const [playIntent,setPlayIntent]=useState(!autoLoad);
  const [mode,setMode]=useState<"original"|"proxy">("original");
  const [revision,setRevision]=useState(0);
  const [src,setSrc]=useState("");
  const [assets,setAssets]=useState<Assets|null>(null);
  const [info,setInfo]=useState<VideoInfo|null>(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [playing,setPlaying]=useState(false);
  const [position,setPosition]=useState(0);
  const [duration,setDuration]=useState(0);
  const inMark=selection?.start||null;
  const outMark=selection?.end||null;
  const [rangeError,setRangeError]=useState("");
  const [poster,setPoster]=useState("");
  const [notice,setNotice]=useState("");
  const fallbackAttempted=useRef(false);
  useEffect(()=>{
    let active=true;setPoster("");
    void invoke<string>("video_poster",{root,path,maxEdge:640,forPlayer:true}).then(value=>{if(active&&typeof value==="string")setPoster(value);}).catch(()=>{});
    return()=>{active=false};
  },[root,path]);
  useEffect(()=>{
    if(!requested)return;
    let active=true;let id="";let timer:ReturnType<typeof setTimeout>|undefined;
    const cancel=()=>{active=false;clearTimeout(timer);if(id)void invoke("cancel_video_preview",{id}).catch(()=>{});};
    cancelJob.current=cancel;
    restore.current=current.current;rangePlaying.current=false;setSrc("");setPlaying(false);setAssets(null);setError("");setBusy(true);
    const poll=async()=>{
      try {
        const status=await invoke<Status>("video_preview_status",{id});
        if(!active)return;
        if(status.state==="failed")throw Error(status.error||"Preview generation failed");
        if(status.state==="complete"&&status.result){
          setAssets(status.result);
          if(status.result.proxy)setSrc(convertFileSrc(status.result.source));
          setBusy(false);return;
        }
        timer=setTimeout(()=>void poll(),400);
      }catch(cause){if(active){setError(String(cause));setBusy(false);cancel();}}
    };
    void (async()=>{
      try {
        if(mode==="original"){
          const source=await invoke<string>("video_source",{root,path});
          if(!active)return;setSrc(convertFileSrc(source));
        }
        id=await invoke<string>("start_video_preview",{root,path,proxy:mode==="proxy"});
        if(!active){void invoke("cancel_video_preview",{id}).catch(()=>{});return;}
        await poll();
      }catch(cause){if(active){setError(String(cause));setBusy(false);cancel();}}
    })();
    return cancel;
  },[root,path,requested,mode,revision]);
  function seek(seconds:number){
    rangePlaying.current=false;
    const value=Math.max(0,Math.min(duration||info?.duration_seconds||0,seconds));
    if(video.current){video.current.currentTime=value;current.current=value;setPosition(value);}
  }
  function toggle(){
    const element=video.current;if(!element)return;
    if(element.paused)void element.play().catch(cause=>setError(`Playback could not start: ${String(cause)}`));else element.pause();
  }
  function playbackFailure(code:number,detail=""){
    if(mode==="original"&&(code===3||code===4)&&!info?.hdr&&!fallbackAttempted.current){
      fallbackAttempted.current=true;setError("");setNotice("This video needs a compatible preview for playback here. Creating it now; the original stays unchanged.");setMode("proxy");return;
    }
    setError(`${mode==="proxy"?"The compatible preview could not play":"The original video could not play"} (media error ${code||"unknown"})${detail?`: ${detail}`:"."} ${info?.hdr?"HDR conversion is unavailable. ":""}Try the Windows player${mode==="original"&&!info?.hdr?" or Create compatible preview":""}.`);
  }
  function setTrim(which:"in"|"out"){
    if(!selection)return;
    const value=markTime(current.current,info?.time_base||null);
    const seconds=markSeconds(value);
    if((which==="in"&&outMark&&seconds>=markSeconds(outMark))||(which==="out"&&inMark&&seconds<=markSeconds(inMark))){
      setRangeError("Trim end must be after trim start.");return;
    }
    setRangeError("");
    selection.onChange(which==="in"?value:selection.start,which==="out"?value:selection.end);
  }
  function keyboard(event:KeyboardEvent<HTMLElement>){
    const target=event.target as HTMLElement;
    if(target.tagName!=="VIDEO"&&target!==event.currentTarget)return;
    if(event.ctrlKey||event.altKey||event.metaKey)return;
    if(event.key===" "){event.preventDefault();toggle();}
    else if(event.key==="ArrowLeft"||event.key==="ArrowRight"){event.preventDefault();seek(current.current+(event.key==="ArrowLeft"?-1:1)*(event.shiftKey?10:5));}
    else if(event.key==="Home"){event.preventDefault();seek(0);}
    else if(event.key==="End"){event.preventDefault();seek(duration);}
  }
  return <section className="video-player" aria-label="Video playback" tabIndex={0} onKeyDown={keyboard}>
    {!src&&poster&&<button className="video-poster" disabled={busy} aria-label={requested?"Retry video preview":"Play selected video"} onClick={()=>requested?setRevision(v=>v+1):setRequested(true)}><img src={poster} alt="Selected video preview"/></button>}
    {!requested&&<button className="secondary-button" onClick={()=>setRequested(true)}>Play Live Photo / video</button>}
    {src&&<><video ref={video} src={src} poster={poster||assets?.filmstrip[0]?.image} autoPlay={playIntent} controls playsInline preload="metadata" aria-label="Selected video"
      onLoadedMetadata={()=>{const element=video.current;if(!element)return;if(Number.isFinite(element.duration))setDuration(element.duration);element.currentTime=Math.min(restore.current,element.duration||0);setPosition(element.currentTime);current.current=element.currentTime;}}
      onDurationChange={()=>{if(video.current&&Number.isFinite(video.current.duration))setDuration(video.current.duration);}}
      onLoadedData={event=>{if(event.currentTarget.videoWidth===0&&event.currentTarget.videoHeight===0)playbackFailure(4,"The file loaded without a decodable video track");}}
      onTimeUpdate={()=>{if(video.current){current.current=video.current.currentTime;if(rangePlaying.current&&current.current>=(outMark?markSeconds(outMark):duration)){video.current.pause();video.current.currentTime=outMark?markSeconds(outMark):duration;current.current=video.current.currentTime;rangePlaying.current=false;}setPosition(current.current);}}}
      onPlay={()=>{setPlaying(true);setPlayIntent(true);}} onPause={()=>setPlaying(false)} onEnded={()=>setPlaying(false)}
      onError={event=>playbackFailure(event.currentTarget.error?.code||0,event.currentTarget.error?.message||"")}/>
      <div className="video-transport">
        <button className="secondary-button" onClick={toggle}>{playing?"Pause":"Play"}</button>
        <button className="icon-button" title="Back 5 seconds" aria-label="Back 5 seconds" onClick={()=>seek(current.current-5)}><Rewind size={18}/></button>
        <button className="icon-button" title="Forward 5 seconds" aria-label="Forward 5 seconds" onClick={()=>seek(current.current+5)}><FastForward size={18}/></button>
        <output aria-label="Playback time">{clockTime(position)} / {clockTime(duration)}</output>
      </div>
      <input className="video-scrubber" type="range" aria-label="Seek video" min={0} max={duration||info?.duration_seconds||0} step={0.001} value={Math.min(position,duration||info?.duration_seconds||0)} onChange={event=>seek(Number(event.target.value))}/>
      {selection&&<div className="video-markers">
        <button className="secondary-button" onClick={()=>setTrim("in")}>Set trim start</button><button className="secondary-button" onClick={()=>setTrim("out")}>Set trim end</button>
        <button className="secondary-button" onClick={()=>{seek(markSeconds(selection.start));rangePlaying.current=true;void video.current?.play().catch(cause=>{rangePlaying.current=false;setError(String(cause));});}}>Preview trim</button>
        <span>Trim: {clockTime(markSeconds(selection.start))} → {clockTime(markSeconds(selection.end))}</span>
        <button className="secondary-button" onClick={()=>{rangePlaying.current=false;selection.onChange(markTime(0,info?.time_base||null),markTime(info?.duration_seconds||duration,info?.time_base||null));setRangeError("");}}>Reset trim</button>
      </div>}
      <small>Focus the player: Space to play/pause, arrows to seek 5 seconds, Shift+arrows for 10 seconds.</small>
    </>}
    {rangeError&&<p role="alert">{rangeError}</p>}
    {!!assets?.filmstrip.length&&<div className="video-filmstrip" aria-label="Video filmstrip">{assets.filmstrip.map(frame=><button key={frame.seconds} aria-label={`Seek to ${clockTime(frame.seconds)}`} onClick={()=>seek(frame.seconds)}><img src={frame.image} alt=""/><span>{clockTime(frame.seconds)}</span></button>)}</div>}
    <VideoDetails root={root} path={path} onInfo={setInfo}/>
    {busy&&<p role="status">{mode==="proxy"?"Creating compatible preview…":"Creating filmstrip…"} <button className="secondary-button" onClick={()=>{cancelJob.current();setBusy(false);}}>Cancel</button></p>}
    {error&&<ErrorNotice title="This video preview could not play" error={error}><button className="secondary-button" onClick={()=>setRevision(v=>v+1)}>Retry preview</button></ErrorNotice>}
    {notice&&busy&&<p role="status">{notice}</p>}
    {assets?.warnings.map(warning=><p key={warning}>{warning}</p>)}
    {requested&&<div className="video-transport">
      <button className="secondary-button" disabled={busy||!!info?.hdr} onClick={()=>{setPlayIntent(playing);setMode("proxy");setRevision(v=>v+1);}}>Create compatible preview</button>
      {mode==="proxy"&&<button className="secondary-button" onClick={()=>{setPlayIntent(playing);setMode("original");setRevision(v=>v+1);}}>Use original video</button>}
      {mode==="original"&&!busy&&!assets&&<button className="secondary-button" onClick={()=>setRevision(v=>v+1)}>Retry filmstrip</button>}
      <button className="secondary-button" onClick={onOpenExternal}>Open in Windows player</button>
    </div>}
    {assets?.proxy&&<p className="results-note">Playing a smaller compatible preview. Original timing and orientation are retained; the original file is unchanged.</p>}
  </section>;
}
