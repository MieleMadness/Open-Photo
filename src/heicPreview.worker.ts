import buildLibheif from "heic-preview-decoder";
import {previewDimensions} from "./previewDimensions";

self.onmessage=async(event:MessageEvent<{encoded:string;maxEdge:number}>)=>{
  const lib=buildLibheif();let decoder:InstanceType<typeof lib.HeifDecoder>|undefined,images:ReturnType<InstanceType<typeof lib.HeifDecoder>["decode"]>=[];
  try{
    const raw=atob(event.data.encoded),bytes=Uint8Array.from(raw,character=>character.charCodeAt(0));
    decoder=new lib.HeifDecoder();images=decoder.decode(bytes);
    const image=images.find(item=>item.is_primary())||images[0];
    if(!image)throw Error("No HEIC image found");
    const width=image.get_width(),height=image.get_height();
    const size=previewDimensions(width,height,event.data.maxEdge);
    // Inspect decoder dimensions before allocating decoded RGBA storage.
    const data=new ImageData(width,height);data.data.fill(255);
    const rendered=await new Promise<ImageData>((resolve,reject)=>image.display(data,value=>value?resolve(value):reject(Error("HEIC decoding failed"))));
    const bitmap=await createImageBitmap(rendered,{resizeWidth:size.width,resizeHeight:size.height,resizeQuality:"high"});
    try{
      const canvas=new OffscreenCanvas(size.width,size.height),context=canvas.getContext("2d");
      if(!context)throw Error("HEIC canvas unavailable");
      context.drawImage(bitmap,0,0);
      const blob=await canvas.convertToBlob({type:"image/jpeg",quality:.8});
      if(blob.size>8*1024*1024)throw Error("HEIC preview exceeds its output limit");
      const result=new Uint8Array(await blob.arrayBuffer());let binary="";
      for(let offset=0;offset<result.length;offset+=8192)binary+=String.fromCharCode(...result.subarray(offset,offset+8192));
      self.postMessage({data:"data:image/jpeg;base64,"+btoa(binary)});
    }finally{bitmap.close();}
  }catch(error){self.postMessage({error:String(error)});}
  finally{for(const image of images)image.free();if(decoder?.decoder)lib.heif_context_free(decoder.decoder);}
};
