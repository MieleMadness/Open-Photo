import {validMapLocations,type MapLocation} from "./mapLocations";
export type ZoomLocationGroup={lat:number;lon:number;photos:MapLocation[]};
/** Approximate Web Mercator pixel buckets at a given zoom, without external plugins. */
export function clusterMapLocations(points:readonly MapLocation[],zoom:number):ZoomLocationGroup[]{
  const z=Math.max(2,Math.min(19,Math.floor(Number.isFinite(zoom)?zoom:2)));
  const scale=256*2**z,cell=48;
  const groups=new Map<string,ZoomLocationGroup>();
  for(const point of validMapLocations(points)){
    const lat=Math.max(-85,Math.min(85,point.lat));
    const sin=Math.sin(lat*Math.PI/180);
    const x=(point.lon+180)/360*scale;
    const y=(0.5-Math.log((1+sin)/(1-sin))/(4*Math.PI))*scale;
    const key=`${Math.floor(x/cell)}:${Math.floor(y/cell)}`;
    let group=groups.get(key);
    if(!group){group={lat:point.lat,lon:point.lon,photos:[]};groups.set(key,group);}
    group.photos.push(point);
  }
  return [...groups.values()];
}
