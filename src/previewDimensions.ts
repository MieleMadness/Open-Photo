/** Match the ordinary native decoder's edge/pixel policy before allocating RGBA. */
export function previewDimensions(width:number,height:number,maxEdge:number){
  if(!Number.isSafeInteger(width)||!Number.isSafeInteger(height)||width<1||height<1||width>20000||height>20000||width*height>50000000)throw Error("Image exceeds the 20,000-pixel edge or 50-megapixel preview limit");
  const edge=Math.max(64,Math.min(1920,maxEdge)),scale=Math.min(1,edge/width,edge/height);
  return {width:Math.max(1,Math.round(width*scale)),height:Math.max(1,Math.round(height*scale))};
}
