import {useEffect,useLayoutEffect,useRef,useState,type ReactNode} from "react";
import {Maximize,RotateCcw,RotateCw,ZoomIn,ZoomOut} from "lucide-react";
type View={scale:number;x:number;y:number};
export function boundImageView(view:View,width:number,height:number,imageWidth:number,imageHeight:number):View{
  const scale=Math.max(.05,Math.min(16,view.scale)),fit=Math.min(width/(imageWidth||width||1),height/(imageHeight||height||1));
  const x=Math.max(0,(imageWidth*fit*scale-width)/2),y=Math.max(0,(imageHeight*fit*scale-height)/2);
  return {scale,x:Math.max(-x,Math.min(x,view.x)),y:Math.max(-y,Math.min(y,view.y))};
}
/** Presentation-only zoom: original/edit coordinates never depend on the viewport. */
export function ImageViewport({identity,children,onRotate}:{identity:string;children:ReactNode;onRotate?:(degrees:-90|90)=>void}){
 const stage=useRef<HTMLDivElement>(null),drag=useRef<{id:number;x:number;y:number;view:View}|null>(null);
 const [size,setSize]=useState({width:1,height:1,iw:1,ih:1}),[view,setView]=useState<View>({scale:1,x:0,y:0});
 const bounded=(value:View)=>boundImageView(value,size.width,size.height,size.iw,size.ih);
 function measure(){const host=stage.current;if(!host)return;const image=host.querySelector("img");setSize({width:Math.max(1,host.clientWidth-24),height:Math.max(1,host.clientHeight-24),iw:image?.naturalWidth||1,ih:image?.naturalHeight||1});}
 useLayoutEffect(()=>{measure();const observer=typeof ResizeObserver==="undefined"?null:new ResizeObserver(measure);if(stage.current)observer?.observe(stage.current);return()=>observer?.disconnect();},[]);
 useEffect(()=>{setView({scale:1,x:0,y:0});drag.current=null;measure();},[identity]);
 useEffect(()=>setView(old=>boundImageView(old,size.width,size.height,size.iw,size.ih)),[size]);
 // A native non-passive listener can consume wheel events only inside an enlarged image.
 useEffect(()=>{const host=stage.current;if(!host)return;const wheel=(e:WheelEvent)=>{if(e.ctrlKey||e.metaKey)return;if(e.deltaY>0&&view.scale<=1)return;e.preventDefault();const rect=host.getBoundingClientRect(),x=e.clientX-rect.left-rect.width/2,y=e.clientY-rect.top-rect.height/2;
   setView(old=>{const scale=Math.max(1,Math.min(16,old.scale*Math.exp(-Math.max(-100,Math.min(100,e.deltaY))*.008))),ratio=scale/old.scale;return bounded({scale,x:x-(x-old.x)*ratio,y:y-(y-old.y)*ratio});});};host.addEventListener("wheel",wheel,{passive:false});return()=>host.removeEventListener("wheel",wheel);},[view.scale,size]);
 function zoom(factor:number){setView(old=>bounded({...old,scale:old.scale*factor}));}
 return <section className="image-viewport" aria-label="Image preview">
  <div className="image-viewport-tools"><button className="icon-button" aria-label="Zoom out image" title="Zoom out" onClick={()=>zoom(1/1.25)}><ZoomOut size={16}/></button><output aria-label="Image zoom">{Math.round(view.scale*100)}% of fit</output><button className="icon-button" aria-label="Zoom in image" title="Zoom in" onClick={()=>zoom(1.25)}><ZoomIn size={16}/></button><button className="secondary-button" onClick={()=>setView({scale:1,x:0,y:0})}><Maximize size={14}/>Fit</button><button className="secondary-button" onClick={()=>setView(bounded({scale:Math.max(.05,Math.min(16,1/Math.min(size.width/size.iw,size.height/size.ih))),x:0,y:0}))} title="Show the bounded preview at native pixels">100% preview</button>{onRotate&&<><button className="icon-button" aria-label="Rotate image left" title="Rotate left 90°" onClick={()=>onRotate(-90)}><RotateCcw size={16}/></button><button className="icon-button" aria-label="Rotate image right" title="Rotate right 90°" onClick={()=>onRotate(90)}><RotateCw size={16}/></button></>}</div>
  <div ref={stage} className="image-viewport-stage" tabIndex={0} aria-label="Zoomable image" onLoadCapture={measure} onKeyDown={e=>{if(e.key==="+"||e.key==="="){e.preventDefault();zoom(1.25);}else if(e.key==="-"){e.preventDefault();zoom(1/1.25);}else if(e.key==="0"){e.preventDefault();setView({scale:1,x:0,y:0});}else if(view.scale>1&&["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(e.key)){e.preventDefault();e.stopPropagation();setView(old=>bounded({...old,x:old.x+(e.key==="ArrowLeft"?40:e.key==="ArrowRight"?-40:0),y:old.y+(e.key==="ArrowUp"?40:e.key==="ArrowDown"?-40:0)}));}}} onPointerDown={e=>{if(view.scale<=1||e.button!==0)return;e.preventDefault();e.currentTarget.setPointerCapture(e.pointerId);drag.current={id:e.pointerId,x:e.clientX,y:e.clientY,view};}} onPointerMove={e=>{const start=drag.current;if(start?.id===e.pointerId)setView(bounded({...start.view,x:start.view.x+e.clientX-start.x,y:start.view.y+e.clientY-start.y}));}} onPointerUp={()=>{drag.current=null;}} onPointerCancel={()=>{drag.current=null;}} onLostPointerCapture={()=>{drag.current=null;}}>
   <div className="image-viewport-content" style={{transform:`translate(${view.x}px,${view.y}px) scale(${view.scale})`}}>{children}</div>
  </div>
 </section>;
}
