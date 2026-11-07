/** Dedicated worker ownership makes cancel/timeout release the decoder and its memory. */
export function decodeHeicPreview(encoded:string,maxEdge:number,signal:AbortSignal):Promise<string>{
  if(signal.aborted)return Promise.reject(new DOMException("Cancelled","AbortError"));
  return new Promise((resolve,reject)=>{
    const worker=new Worker(new URL("./heicPreview.worker.ts",import.meta.url),{type:"module"});
    const finish=(error?:unknown,value?:string)=>{clearTimeout(timer);signal.removeEventListener("abort",cancel);worker.terminate();if(error)reject(error);else resolve(value!);};
    const cancel=()=>finish(new DOMException("Cancelled","AbortError"));
    const timer=setTimeout(()=>finish(Error("HEIC preview timed out")),30000);
    signal.addEventListener("abort",cancel,{once:true});
    worker.onerror=()=>finish(Error("HEIC decoder failed"));
    worker.onmessage=(event:MessageEvent<{data?:string;error?:string}>)=>event.data.error?finish(Error(event.data.error)):event.data.data?finish(undefined,event.data.data):finish(Error("HEIC decoder returned no preview"));
    worker.postMessage({encoded,maxEdge});
  });
}
