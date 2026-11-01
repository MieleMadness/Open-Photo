import { useEffect, useRef } from "react";
import L from "leaflet";
import {clusterMapLocations} from "./mapZoomClusters";
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
    const bounds:L.LatLngExpression[]=points.filter(point=>Number.isFinite(point.lat)&&Number.isFinite(point.lon)&&Math.abs(point.lat)<=90&&Math.abs(point.lon)<=180).map(point=>[point.lat,point.lon]);
    const markers=L.layerGroup().addTo(map);
    const renderMarkers=()=>{
      markers.clearLayers();
      for(const group of clusterMapLocations(points,map.getZoom())){
        const count=group.photos.length;
        let nextPhoto=0;
        const marker=L.circleMarker([group.lat,group.lon],{radius:count>1?Math.min(19,9+Math.log2(count)*2):7,color:"#6727ad",fillColor:"#28b678",fillOpacity:.85,weight:2}).addTo(markers);
        marker.bindTooltip(count===1?(group.photos[0].path.split(/[\\/]/).pop()||"Photo"):`${count} photos · Click to browse`);
        marker.on("click",()=>{onChoose(group.photos[nextPhoto%count].path);nextPhoto++;});
      }
    };
    // Initialize the view once; a second setView can leave a zoom running during teardown.
    if(bounds.length) map.fitBounds(L.latLngBounds(bounds),{padding:[48,48],maxZoom:15,animate:false});
    else map.setView([27.95,-82.46],6);
    renderMarkers();
    map.on("zoomend",renderMarkers);
    return ()=>{window.clearTimeout(initialResize);observer?.disconnect();map.off("zoomend",renderMarkers);map.remove();};
  },[points,onChoose,compact]);
  return <div ref={container} className="photo-location-map" aria-label="Photo locations map"/>;
}
