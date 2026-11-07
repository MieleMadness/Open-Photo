import { useEffect, useMemo, useRef, useState } from "react";
import L from "leaflet";
import {clusterMapLocations} from "./mapZoomClusters";
import {validMapLocations} from "./mapLocations";
import "leaflet/dist/leaflet.css";
export type PhotoLocation = {path:string;lat:number;lon:number};
export default function PhotoMap({points,onChoose,compact=false}:{points:PhotoLocation[];onChoose:(path:string)=>void;compact?:boolean}) {
  const container = useRef<HTMLDivElement>(null);
  const [query,setQuery]=useState(""),[offset,setOffset]=useState(0);
  const locations=useMemo(()=>validMapLocations(points).filter(point=>point.path.toLowerCase().includes(query.toLowerCase())),[points,query]);
  const pageOffset=Math.min(offset,Math.max(0,Math.floor((locations.length-1)/50)*50));
  // Keep the latest selection handler without rebuilding Leaflet when the parent renders.
  const onChooseRef = useRef(onChoose);
  onChooseRef.current = onChoose;
  useEffect(()=>{
    if(!container.current) return;
    const map = L.map(container.current,{scrollWheelZoom:!compact,minZoom:2,maxBounds:[[-85,-180],[85,180]],maxBoundsViscosity:1,worldCopyJump:false});
    L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {minZoom:2,maxZoom:19,noWrap:true,bounds:[[-85,-180],[85,180]],attribution:'© OpenStreetMap contributors'}).addTo(map);
    // Sidebar width changes resize the map without changing its media or view.
    const resize=()=>map.invalidateSize({pan:false});
    const initialResize=window.setTimeout(resize,0);
    const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(resize):null;
    observer?.observe(container.current);
    const bounds:L.LatLngExpression[]=validMapLocations(points).map(point=>[Math.max(-85,Math.min(85,point.lat)),point.lon]);
    const markers=L.layerGroup().addTo(map);
    const renderMarkers=()=>{
      const focused=container.current?.contains(document.activeElement)?(document.activeElement as HTMLElement).dataset.mediaPath:undefined;
      markers.clearLayers();
      for(const group of clusterMapLocations(points,map.getZoom())){
        const count=group.photos.length;
        let nextPhoto=0;
        const marker=L.circleMarker([group.lat,group.lon],{radius:count>1?Math.min(19,9+Math.log2(count)*2):7,color:"var(--map-marker-border)",fillColor:"var(--map-marker-fill)",fillOpacity:.85,weight:2}).addTo(markers);
        const label=document.createElement("span");
        label.textContent=count===1?(group.photos[0].path.split(/[\\/]/).pop()||"Media"):`${count} items · Click to browse`;
        marker.bindTooltip(label);
        const activate=()=>{onChooseRef.current(group.photos[nextPhoto%count].path);nextPhoto++;};
        if(!compact)marker.on("click",activate);
        const element=marker.getElement?.();
        if(element&&!compact){
          element.setAttribute("tabindex","0");element.setAttribute("role","button");
          element.setAttribute("aria-label",count===1?`Open ${label.textContent}`:`Browse ${count} media items at this location; activate again for the next item. All items are also in Browse locations.`);
          (element as SVGElement).dataset.mediaPath=group.photos[0].path;
          element.addEventListener("keydown",event=>{const key=(event as KeyboardEvent).key;if(key==="Enter"||key===" "){event.preventDefault();event.stopPropagation();activate();}});
          if(focused&&group.photos.some(photo=>photo.path===focused))(element as SVGElement).focus();
        }
      }
    };
    // Initialize the view once; a second setView can leave a zoom running during teardown.
    if(bounds.length) map.fitBounds(L.latLngBounds(bounds),{padding:[48,48],maxZoom:15,animate:false});
    else map.setView([0,0],2);
    renderMarkers();
    map.on("zoomend",renderMarkers);
    return ()=>{window.clearTimeout(initialResize);observer?.disconnect();map.off("zoomend",renderMarkers);map.remove();};
  },[points,compact]);
  return <><div ref={container} className="photo-location-map" aria-label="Media locations map"/>{!compact&&<details className="map-location-list"><summary>Browse locations ({locations.length})</summary><p>Open any matching item using the keyboard or pointer. Coordinates are available without map imagery.</p><label>Filter location files<input type="search" value={query} onChange={event=>{setQuery(event.target.value);setOffset(0);}}/></label><ul>{locations.slice(pageOffset,pageOffset+50).map(point=><li key={point.path}><button className="secondary-button" onClick={()=>onChooseRef.current(point.path)}><span>{point.path}</span><small>{point.lat.toFixed(5)}, {point.lon.toFixed(5)}</small></button></li>)}</ul><div className="feedback-actions"><button className="secondary-button" disabled={pageOffset===0} onClick={()=>setOffset(pageOffset-50)}>Previous locations</button><span role="status">{locations.length?`${pageOffset+1}–${Math.min(pageOffset+50,locations.length)} of ${locations.length}`:"No matching locations"}</span><button className="secondary-button" disabled={pageOffset+50>=locations.length} onClick={()=>setOffset(pageOffset+50)}>Next locations</button></div></details>}</>;
}
