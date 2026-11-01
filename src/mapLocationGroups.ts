import {validMapLocations,type MapLocation} from "./mapLocations";
export type LocationGroup={lat:number;lon:number;photos:MapLocation[]};
/** Group photos within roughly one metre so overlapping markers remain selectable. */
export function groupNearbyLocations(points:readonly MapLocation[]):LocationGroup[]{
  const groups=new Map<string,LocationGroup>();
  for(const point of validMapLocations(points)){
    const key=`${point.lat.toFixed(5)}:${point.lon.toFixed(5)}`;
    let group=groups.get(key);
    if(!group){group={lat:point.lat,lon:point.lon,photos:[]};groups.set(key,group);}
    group.photos.push(point);
  }
  return [...groups.values()];
}
