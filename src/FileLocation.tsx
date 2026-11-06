import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import PhotoMap,{type PhotoLocation} from "./PhotoMap";

const ignoreMarker=()=>{};
export default function FileLocation({root,path,extension,modified,desktop}:{root:string;path:string;extension:string;modified:number;desktop:boolean}){
  const supported=["jpg","jpeg","tif","tiff"].includes(extension.toLowerCase());
  const identity=JSON.stringify([root,path,modified]);
  const [state,setState]=useState<{identity:string;points:PhotoLocation[];error:boolean}>();
  const [retry,setRetry]=useState(0);
  useEffect(()=>{
    if(!desktop||!supported)return;
    let active=true;
    setState(undefined);
    void invoke<PhotoLocation[]>("gps_for_photos",{root,paths:[path]}).then(points=>{
      if(active)setState({identity,points:points.filter(point=>point.path===path&&Number.isFinite(point.lat)&&Number.isFinite(point.lon)&&Math.abs(point.lat)<=90&&Math.abs(point.lon)<=180),error:false});
    }).catch(()=>{if(active)setState({identity,points:[],error:true});});
    return()=>{active=false;};
  },[identity,root,path,desktop,supported,retry]);
  const current=state?.identity===identity?state:undefined;
  return <section className="file-location" aria-label="File location">
    <h3>Location</h3>
    {!supported?<p>Location metadata is currently supported for JPEG and TIFF photos. Support for other formats is planned.</p>:
      !desktop?<p>Open this file in the desktop app to read its GPS location.</p>:
      !current?<p role="status">Reading GPS location…</p>:
      current.error?<><p role="alert">The location could not be read.</p><button className="secondary-button" onClick={()=>setRetry(value=>value+1)}>Retry location</button></>:
      !current.points.length?<p>No GPS location was found in this file.</p>:
      <><PhotoMap points={current.points} onChoose={ignoreMarker} compact/><p>{current.points[0].lat.toFixed(5)}, {current.points[0].lon.toFixed(5)}</p><p>Map imagery requires an internet connection. Coordinates remain available offline.</p></>}
  </section>;
}
