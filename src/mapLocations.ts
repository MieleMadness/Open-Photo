export type MapLocation={path:string;lat:number;lon:number};
/** Filter malformed GPS values before handing them to Leaflet. */
export function validMapLocations(points:readonly MapLocation[]):MapLocation[]{
  return points.filter(point=>Number.isFinite(point.lat)&&Number.isFinite(point.lon)&&Math.abs(point.lat)<=90&&Math.abs(point.lon)<=180&&typeof point.path==="string"&&point.path.length>0);
}
