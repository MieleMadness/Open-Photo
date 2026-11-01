import { useEffect, useRef } from "react";
import L from "leaflet";
import {groupNearbyLocations} from "./mapLocationGroups";
import "leaflet/dist/leaflet.css";
export type PhotoLocation = {path:string;lat:number;lon:number};
export default function PhotoMap({points,onChoose,compact=false}:{points:PhotoLocation[];onChoose:(path:string)=>void;compact?:boolean}) {
  const container = useRef<HTMLDivElement>(null);
  useEffect(()=>{
    if(!container.current) return;
    const map = L.map(container.current,{scrollWheelZoom:!compact,minZoom:2,maxBounds:[[-85,-180],[85,180]],maxBoundsViscosity:1,worldCopyJump:false});
    L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {minZoom:2,maxZoom:19,noWrap:true,bounds:[[-85,-180],[85,180]],attribution:'© OpenStreetMap contributors'}).addTo(map);
    // Sidebar width changes resize the map without changing its media or view.
    const resize=()=>map.invalidateSize({pan:false});
    const initialResize=window.setTimeout(resize,0);
    const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(resize):null;
    observer?.observe(container.current);
    const bounds:L.LatLngExpression[] = [];
    for(const group of groupNearbyLocations(points)) {
      const location:L.LatLngExpression=[group.lat,group.lon];
      bounds.push(location);
      let nextPhoto=0;
      const count=group.photos.length;
      const marker=L.circleMarker(location,{radius:count>1?11:7,color:"#6727ad",fillColor:"#28b678",fillOpacity:.85,weight:2}).addTo(map);
      marker.bindTooltip(count===1?(group.photos[0].path.split(/[\\/]/).pop()||"Photo"):`${count} photos · Click to browse`);
      marker.on("click",()=>{onChoose(group.photos[nextPhoto%count].path);nextPhoto++;});
    }
    // Initialize the view once; a second setView can leave a zoom running during teardown.
    if(bounds.length) map.fitBounds(L.latLngBounds(bounds),{padding:[48,48],maxZoom:15,animate:false});
    else map.setView([27.95,-82.46],6);
    return ()=>{window.clearTimeout(initialResize);observer?.disconnect();map.remove();};
  },[points,onChoose,compact]);
  return <div ref={container} className="photo-location-map" aria-label="Photo locations map"/>;
}
