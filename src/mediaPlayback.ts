import {saveSetting} from "./userData";
import {useCallback,useRef} from "react";

const volumeKey="openphoto.videoVolume";
export function savedVideoVolume(){
  const value=Number(localStorage.getItem(volumeKey));
  return Number.isFinite(value)&&value>=0&&value<=1?value:1;
}
export function saveVideoVolume(value:number){
  const next=Math.max(0,Math.min(1,value));
  saveSetting(volumeKey,String(next));
  window.dispatchEvent(new CustomEvent("openphoto-video-volume",{detail:next}));
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
  return [video,bindVideo] as const;
}
