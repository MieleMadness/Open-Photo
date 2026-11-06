/** Visible row range for a fixed-height virtual grid/list. Keeps overscan near the end. */
export function virtualRows(totalRows:number,rowHeight:number,scrollTop:number,viewportHeight:number,before=3,after=6){
  if(!Number.isFinite(totalRows)||totalRows<=0)return {start:0,end:0};
  const count=Math.floor(totalRows);
  const height=Math.max(1,rowHeight);
  const first=Math.min(count-1,Math.max(0,Math.floor(Math.max(0,scrollTop)/height)));
  const start=Math.max(0,first-before);
  const end=Math.min(count,Math.max(first+1,start+Math.ceil(Math.max(0,viewportHeight)/height)+before+after));
  return {start,end};
}
