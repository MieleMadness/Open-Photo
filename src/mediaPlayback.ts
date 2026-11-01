import {saveSetting} from "./userData";
import {useCallback,useEffect,useRef} from "react";

const volumeKey="openphoto.videoVolume";
const volumeEvent="openphoto-video-volume";
export function savedVideoVolume(){
  const saved=localStorage.getItem(volumeKey);
  if(saved===null||!saved.trim())return 1;
  const value=Number(saved);
  return Number.isFinite(value)&&value>=0&&value<=1?value:1;
}
export function saveVideoVolume(value:number){
  if(!Number.isFinite(value))return;
  const next=Math.max(0,Math.min(1,value));
  saveSetting(volumeKey,String(next));
  window.dispatchEvent(new CustomEvent<number>(volumeEvent,{detail:next}));
}
export function applySavedVideoVolume(element:HTMLVideoElement|null){
  if(element)element.volume=savedVideoVolume();
}

/** Release a detached decoder before its job-owned file can be removed or another player starts. */
export function releaseVideo(element:HTMLVideoElement|null){
  if(!element)return;
  element.pause();element.removeAttribute("src");element.load();
}

/** React can detach and reattach the same ref in StrictMode. Release only after ownership changes. */
export function useVideoElement(){
  const video=useRef<HTMLVideoElement>(null);
  const bindVideo=useCallback((element:HTMLVideoElement|null)=>{
    const previous=video.current;video.current=element;if(element)applySavedVideoVolume(element);
    if(previous&&previous!==element)queueMicrotask(()=>{if(video.current!==previous)releaseVideo(previous);});
  },[]);
  useEffect(()=>{
    const syncVolume=(event:Event)=>{
      const next=(event as CustomEvent<number>).detail;
      if(!Number.isFinite(next)||next<0||next>1)return;
      const element=video.current;
      if(element&&element.volume!==next)element.volume=next;
    };
    window.addEventListener(volumeEvent,syncVolume);
    return ()=>window.removeEventListener(volumeEvent,syncVolume);
  },[]);
  return [video,bindVideo] as const;
}
