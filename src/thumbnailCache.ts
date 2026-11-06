/** Bounded data-URL cache for Library previews; source metadata belongs in each caller's key. */
export class ThumbnailCache {
  private entries=new Map<string,{value:string;bytes:number}>();
  private total=0;
  constructor(private readonly maxBytes=24*1024*1024,private readonly maxItems=80){}
  get(key:string):string|undefined{
    const entry=this.entries.get(key);
    if(!entry)return undefined;
    this.entries.delete(key);this.entries.set(key,entry);
    return entry.value;
  }
  set(key:string,value:string){
    const bytes=value.length*2; // Conservative bound for UTF-16 JS strings.
    const previous=this.entries.get(key);
    if(previous){this.total-=previous.bytes;this.entries.delete(key);}
    if(bytes>this.maxBytes)return;
    this.entries.set(key,{value,bytes});this.total+=bytes;
    while(this.total>this.maxBytes||this.entries.size>this.maxItems){
      const oldest=this.entries.keys().next().value;
      if(oldest===undefined)break;
      this.total-=this.entries.get(oldest)!.bytes;this.entries.delete(oldest);
    }
  }
  get size(){return this.entries.size;}
  get estimatedBytes(){return this.total;}
}
