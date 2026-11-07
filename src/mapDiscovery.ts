type Media={path:string};
/** Path membership is linear even when almost every library item has GPS. */
export function mapCaptureYears<T extends Media>(photos:T[],points:Media[],year:(photo:T)=>number){
  const located=new Set(points.map(point=>point.path));
  return [...new Set(photos.filter(photo=>located.has(photo.path)).map(photo=>String(year(photo))))].sort().reverse();
}
