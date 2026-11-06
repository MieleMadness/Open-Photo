/** Fixed output dimensions shared by the editor and its tests. The project persists width/height, not a vendor-specific name. */
export const socialCanvases=[
  {width:1920,height:1080,label:"16:9 · YouTube / landscape"},
  {width:1080,height:1920,label:"9:16 · Reels / TikTok / Shorts"},
  {width:1080,height:1350,label:"4:5 · Portrait feed"},
  {width:1080,height:1080,label:"1:1 · Square feed"},
  {width:3840,height:2160,label:"4K · Landscape"},
  {width:2160,height:3840,label:"4K · Vertical"}
] as const;
export function isValidSocialCanvas(width:number,height:number){
  return socialCanvases.some(item=>item.width===width&&item.height===height);
}
