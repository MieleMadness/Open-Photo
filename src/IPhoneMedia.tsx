import {VideoPlayer} from "./VideoPlayer";
import {LibraryThumbnail} from "./LibraryThumbnail";
export function IPhonePicture({root,path,name,sourceVersion}:{root:string;path:string;name:string;sourceVersion?:string|null}){
  return <LibraryThumbnail root={root} photo={{path,name,source_version:sourceVersion,bytes:0,modified_unix:0,extension:"heic"}} maxEdge={1600}/>;
}
/** A new media identity resets all transient player state. */
export function IPhoneMotion(props:{root:string;path:string;onOpenExternal:()=>void;autoLoad?:boolean;viewerMode?:boolean;startAt?:number}){
  return <VideoPlayer key={props.root+"\0"+props.path} {...props}/>;
}
