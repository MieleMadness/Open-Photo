// HTML playback supplies approximate seconds. Store preview marks as integer source ticks.
export type VideoTime={ticks:string;time_base:string};
export function parseClockTime(value:string):number|null {
  const parts=value.trim().split(":");
  if(parts.length>3||!parts.every((part,index)=>index===parts.length-1?/^\d+(?:\.\d{1,3})?$/.test(part):/^\d+$/.test(part)))return null;
  const numbers=parts.map(Number);
  if(parts.length>1&&numbers[numbers.length-1]>=60)return null;
  if(parts.length===3&&numbers[1]>=60)return null;
  const seconds=numbers.reduce((total,part)=>total*60+part,0);
  return Number.isFinite(seconds)?seconds:null;
}
export function markTime(seconds:number,timeBase:string|null):VideoTime {
  const base=timeBase&&/^[1-9]\d*\/[1-9]\d*$/.test(timeBase)?timeBase:"1/1000000";
  const [numerator,denominator]=base.split("/").map(BigInt);
  const micros=BigInt(Math.round(Math.max(0,seconds)*1_000_000));
  const divisor=numerator*1_000_000n;
  return {ticks:((micros*denominator+divisor/2n)/divisor).toString(),time_base:base};
}
export function markSeconds(time:VideoTime):number {
  const [a,b]=time.time_base.split("/").map(Number);
  return Number(time.ticks)*a/b;
}
export function clockTime(seconds:number):string {
  const millis=Math.max(0,Math.round(seconds*1000));
  const pad=(value:number)=>value.toString().padStart(2,"0");
  return `${pad(Math.floor(millis/3600000))}:${pad(Math.floor(millis/60000)%60)}:${pad(Math.floor(millis/1000)%60)}.${(millis%1000).toString().padStart(3,"0")}`;
}
