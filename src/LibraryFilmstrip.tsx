import {useLayoutEffect,useRef,useState,type ReactNode} from "react";
type Item={path:string;name:string};
/** Library neighbors, not frames from a video. Render only the visible thumbnail window. */
export function LibraryFilmstrip<T extends Item>({items,current,onSelect,thumbnail}:{items:T[];current:string;onSelect:(item:T)=>void;thumbnail:(item:T)=>ReactNode}){
 const host=useRef<HTMLDivElement>(null),drag=useRef<{id:number;x:number;scroll:number;moved:boolean}|null>(null),suppressClick=useRef(false),[left,setLeft]=useState(0),[width,setWidth]=useState(800);const stride=112;
 useLayoutEffect(()=>{const element=host.current;if(!element)return;const measure=()=>setWidth(element.clientWidth||800);measure();const observer=typeof ResizeObserver==="undefined"?null:new ResizeObserver(measure);observer?.observe(element);return()=>observer?.disconnect();},[]);
 useLayoutEffect(()=>{const index=items.findIndex(item=>item.path===current),element=host.current;if(index<0||!element)return;const target=index*stride;if(target<element.scrollLeft||target+stride>element.scrollLeft+width){element.scrollLeft=Math.max(0,target-width/2+stride/2);setLeft(element.scrollLeft);}},[current,items,width]);
 const start=Math.max(0,Math.floor(left/stride)-2),end=Math.min(items.length,start+Math.ceil(width/stride)+4);
 return <section className="viewer-filmstrip" aria-label="Library filmstrip"><div ref={host} className="viewer-filmstrip-scroll" onScroll={e=>setLeft(e.currentTarget.scrollLeft)}
 onWheel={e=>{if(e.ctrlKey||e.metaKey||e.shiftKey)return;const element=e.currentTarget;if(element.scrollWidth<=element.clientWidth)return;if(Math.abs(e.deltaY)>Math.abs(e.deltaX)){element.scrollLeft+=e.deltaY;}}}
 onPointerDown={e=>{if(e.button!==0)return;drag.current={id:e.pointerId,x:e.clientX,scroll:e.currentTarget.scrollLeft,moved:false};}}
 onPointerMove={e=>{const state=drag.current;if(!state||state.id!==e.pointerId)return;const delta=e.clientX-state.x;if(!state.moved&&Math.abs(delta)>5){state.moved=true;e.currentTarget.setPointerCapture(e.pointerId);}if(state.moved)e.currentTarget.scrollLeft=state.scroll-delta;}}
 onPointerUp={()=>{if(drag.current?.moved)suppressClick.current=true;drag.current=null;}}
 onPointerCancel={()=>{drag.current=null;}}
 onLostPointerCapture={()=>{drag.current=null;}}><div style={{position:"relative",width:items.length*stride,height:84}}>{items.slice(start,end).map((item,index)=><button className="viewer-neighbor" key={item.path} aria-label={`View ${item.name}`} aria-current={item.path===current?"true":undefined} title={item.name} onClick={e=>{if(suppressClick.current){suppressClick.current=false;e.preventDefault();return;}onSelect(item);}} style={{position:"absolute",left:(start+index)*stride,width:104}}>{thumbnail(item)}<span>{item.name}</span></button>)}</div></div></section>;
}
