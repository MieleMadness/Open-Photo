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
