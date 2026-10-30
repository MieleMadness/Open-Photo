import {useCallback,useRef} from "react";

/** Release a detached decoder before its job-owned file can be removed or another player starts. */
export function releaseVideo(element:HTMLVideoElement|null){
  if(!element)return;
  element.pause();element.removeAttribute("src");element.load();
}

/** React can detach and reattach the same ref in StrictMode. Release only after ownership changes. */
export function useVideoElement(){
  const video=useRef<HTMLVideoElement>(null);
  const bindVideo=useCallback((element:HTMLVideoElement|null)=>{
    const previous=video.current;video.current=element;
    if(previous&&previous!==element)queueMicrotask(()=>{if(video.current!==previous)releaseVideo(previous);});
  },[]);
  return [video,bindVideo] as const;
}
