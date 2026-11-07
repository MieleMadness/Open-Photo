import {useEffect,useState} from "react";
import {Images} from "lucide-react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
import {libraryPreviews} from "./previewService";
type Media={path:string;name:string;extension:string;bytes:number;modified_unix:number;source_version?:string|null};
export function LibraryThumbnail({root,photo,maxEdge,hoverVideoPath}:{root:string;photo:Media;maxEdge:number;hoverVideoPath?:string}){
  const [src,setSrc]=useState(""),[failed,setFailed]=useState(false),[hovering,setHovering]=useState(false),[hoverSrc,setHoverSrc]=useState("");
  useEffect(()=>{
    setSrc("");setFailed(false);
    if(!root||!("__TAURI_INTERNALS__" in window))return;
    const extension=photo.extension.toLowerCase(),kind=["heic","heif"].includes(extension)?"heic":["mov","mp4","m4v","avi","mkv","webm"].includes(extension)?"video":"image";
    return libraryPreviews.subscribe({root,path:photo.path,maxEdge,sourceVersion:photo.source_version,bytes:photo.bytes,modified:photo.modified_unix,kind},value=>{if(value)setSrc(value);else setFailed(true);});
  },[root,photo.path,photo.extension,photo.source_version,photo.bytes,photo.modified_unix,maxEdge]);
  useEffect(()=>{
    let active=true;setHoverSrc("");
    if(hovering&&hoverVideoPath&&root&&"__TAURI_INTERNALS__" in window)void invoke<string>("video_source",{root,path:hoverVideoPath}).then(path=>{if(active)setHoverSrc(convertFileSrc(path));}).catch(()=>{});
    return()=>{active=false};
  },[hovering,hoverVideoPath,root,photo.source_version]);
  const still=src?<img className="photo-image" src={src} alt={photo.name} loading="lazy"/>:<div className="photo-fallback"><Images size={28}/><small>{failed?"Preview unavailable":"Loading preview…"}</small></div>;
  return hoverVideoPath?<span className="library-hover-preview" onPointerEnter={()=>setHovering(true)} onPointerLeave={()=>setHovering(false)}>{still}{hovering&&hoverSrc&&<video className="library-hover-video" src={hoverSrc} muted loop autoPlay playsInline preload="metadata" aria-label={"Preview "+photo.name}/>}</span>:still;
}
