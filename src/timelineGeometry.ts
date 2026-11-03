/** Keep ruler work bounded even for long durations and highly zoomed timelines. */
export function timelineTicks(duration:number,width:number){
  if(!Number.isFinite(duration)||duration<=0||!Number.isFinite(width)||width<=0)return [];
  const desired=Math.max(duration*80/width,duration/200);
  const steps=[.1,.2,.5,1,2,5,10,15,30,60,120,300,600,900,1800,3600];
  const interval=steps.find(value=>value>=desired)||Math.ceil(desired/3600)*3600;
  return Array.from({length:Math.min(201,Math.floor(duration/interval)+1)},(_,index)=>{
    const seconds=Number((index*interval).toFixed(6));
    const hours=Math.floor(seconds/3600),minutes=Math.floor(seconds/60)%60,remaining=Math.floor(seconds%60);
    const label=interval<1?`${seconds.toFixed(1)}s`:hours?`${hours}:${String(minutes).padStart(2,"0")}:${String(remaining).padStart(2,"0")}`:`${Math.floor(seconds/60)}:${String(remaining).padStart(2,"0")}`;
    return {seconds,left:seconds/duration*100,label};
  });
}

/** Convert an insertion boundary into an index after removing the dragged clip. */
export function clipDropIndex(from:number,target:number,after:boolean){
  const boundary=target+(after?1:0);
  return boundary-(from<boundary?1:0);
}

/** Seek strictly to the previous/next edit boundary, not the nearest repeated frame.
 * The first and final project boundaries are included for predictable keyboard navigation.
 */
export function adjacentCutPosition(position:number,clipEnds:number[],direction:"previous"|"next"):number|null{
  if(!Number.isFinite(position)||clipEnds.length===0||clipEnds.some(end=>!Number.isFinite(end)||end<=0))return null;
  const cuts=[0,...clipEnds];
  const epsilon=1e-6;
  if(direction==="previous"){
    for(let index=cuts.length-1;index>=0;index--)if(cuts[index]<position-epsilon)return cuts[index];
  }else{
    for(const cut of cuts)if(cut>position+epsilon)return cut;
  }
  return null;
}
