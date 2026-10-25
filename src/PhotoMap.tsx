import { useEffect, useRef } from "react";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
export type PhotoLocation = {path:string;lat:number;lon:number};
export default function PhotoMap({points,onChoose}:{points:PhotoLocation[];onChoose:(path:string)=>void}) {
  const container = useRef<HTMLDivElement>(null);
  useEffect(()=>{
    if(!container.current) return;
    const map = L.map(container.current).setView([27.95,-82.46],8);
    L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {maxZoom:19,attribution:'© OpenStreetMap contributors'}).addTo(map);
    const bounds:L.LatLngExpression[] = [];
    for(const point of points) {
      if(!Number.isFinite(point.lat)||!Number.isFinite(point.lon)) continue;
      const location:L.LatLngExpression=[point.lat,point.lon];
      bounds.push(location);
      L.circleMarker(location,{radius:7,color:"#6727ad",fillColor:"#28b678",fillOpacity:.85,weight:2}).addTo(map)
        .bindTooltip(point.path.split(/[\\/]/).pop()||"Photo").on("click",()=>onChoose(point.path));
    }
    if(bounds.length) map.fitBounds(L.latLngBounds(bounds),{padding:[35,35],maxZoom:15});
    return ()=>map.remove();
  },[points,onChoose]);
  return <div ref={container} className="photo-location-map" aria-label="Photo locations map"/>;
}
