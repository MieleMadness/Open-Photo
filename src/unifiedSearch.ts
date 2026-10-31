/** One result identity and evidence-based ranking contract for all discovery providers. */
export type SearchMedia={path:string;name:string;extension:string;modified_unix:number;libraryRoot?:string};
export type MatchKind="filename"|"path"|"tag"|"rating"|"person"|"ocr"|"semantic"|"video_semantic"|"type";
export type SearchEvidence={root:string;path:string;modified_unix:number;kind:"person"|"ocr"|"semantic"|"video_semantic";text:string;matched_terms?:string[];score:number;timestamp_seconds:number|null;source_video_path?:string};
export type MatchReason={kind:MatchKind;label:string;score:number;timestamp_seconds?:number};
export type RankedResult={media_id:string;root:string;path:string;media_type:"photo"|"video";score:number;reasons:MatchReason[];matched_timestamps:number[]};
export type SearchQuery={text:string;terms:string[];tag:string;person:string;path:string;rating:number|null;minimumRating:boolean;favorite:boolean;type:""|"photo"|"video";error:string};
const videoExtensions=new Set(["mov","mp4","m4v","avi","mkv","webm"]);
export const mediaIdentity=(root:string,path:string)=>JSON.stringify([root,path]);
const normalize=(value:string)=>value.normalize("NFKC").toLowerCase().replace(/\s+/gu," ").trim();
export function parseSearch(value:string):SearchQuery{
  const result:SearchQuery={text:"",terms:[],tag:"",person:"",path:"",rating:null,minimumRating:false,favorite:false,type:"",error:""};
  if(value.length>500){result.error="Keep searches within 500 characters.";return result;}
  const free:string[]=[];
  // Quotes group names/phrases; only known field names have special meaning (Windows paths remain text).
  const tokens=value.match(/(?:[^\s"']+|"[^"]*"|'[^']*')+/gu)||[];
  for(const token of tokens){
    const match=/^(tag|person|path|rating|is|type):(.*)$/iu.exec(token),unquote=(text:string)=>text.replace(/^(["'])(.*)\1$/u,"$2");
    if(!match){free.push(unquote(token));continue;}
    const field=match[1].toLowerCase(),text=unquote(match[2]);
    if(!text){result.error=`Enter a value after ${field}:`;continue;}
    if(field==="tag"||field==="person"||field==="path")result[field]=normalize(text);
    else if(field==="type"&&(text.toLowerCase()==="photo"||text.toLowerCase()==="video"))result.type=text.toLowerCase() as "photo"|"video";
    else if(field==="is"&&text.toLowerCase()==="favorite")result.favorite=true;
    else if(field==="rating"&&/^(?:>=)?[0-5]$/u.test(text)){result.rating=Number(text.replace(">=",""));result.minimumRating=text.startsWith(">=");}
    else result.error=`Use type:photo, type:video, is:favorite, or rating:0–5 / rating:>=0–5.`;
  }
  result.text=normalize(free.join(" "));result.terms=free.map(normalize).filter(Boolean);if(result.person.length>200)result.error="Keep person names within 200 characters.";if(result.terms.length>50)result.error="Use at most 50 search terms.";return result;
}
const containsTerms=(text:string,terms:string[])=>terms.every(term=>normalize(text).includes(term));
export function rankSearch(photos:SearchMedia[],root:string,query:SearchQuery,tags:Record<string,string[]>,ratings:Record<string,number>,evidence:SearchEvidence[]):RankedResult[]{
  if(query.error)return [];
  const indexed=new Map<string,SearchEvidence[]>();
  for(const item of evidence){if(!Number.isFinite(item.score))continue;const key=mediaIdentity(item.root,item.path),items=indexed.get(key)||[];items.push(item);indexed.set(key,items);}
  const results:RankedResult[]=[];
  for(const photo of photos){
    const parent=photo.libraryRoot||root,id=mediaIdentity(parent,photo.path),type=videoExtensions.has(photo.extension.toLowerCase())?"video":"photo",rating=ratings[photo.path]||0,keywords=tags[photo.path]||[];
    if(query.type&&type!==query.type||query.favorite&&rating!==5||query.rating!==null&&(query.minimumRating?rating<query.rating:rating!==query.rating)||query.path&&!normalize(photo.path).includes(query.path)||query.tag&&!keywords.some(t=>normalize(t)===query.tag||normalize(t).startsWith(query.tag+"/")))continue;
    const reasons:MatchReason[]=[],current=(indexed.get(id)||[]).filter(e=>e.modified_unix===photo.modified_unix);
    const people=current.filter(e=>e.kind==="person");if(query.person&&!people.some(e=>normalize(e.text).includes(query.person)))continue;
    const corpus=[photo.name,photo.path,...keywords,...current.filter(e=>e.kind==="ocr"||e.kind==="person").flatMap(e=>[e.text,...e.matched_terms||[]])].join(" ");
    const semantic=query.text?current.filter(e=>e.score>=0.2&&(e.kind==="semantic"?type==="photo"&&e.timestamp_seconds===null:e.kind==="video_semantic"&&(type==="video"||!!e.source_video_path)&&e.timestamp_seconds!==null&&Number.isFinite(e.timestamp_seconds)&&e.timestamp_seconds>=0)):[];
    if(query.terms.length&&!containsTerms(corpus,query.terms)&&!semantic.length)continue;
    const add=(kind:MatchKind,label:string,score:number,timestamp?:number)=>reasons.push({kind,label,score,...timestamp!==undefined?{timestamp_seconds:timestamp}:{}});
    if(query.terms.length){
      const name=normalize(photo.name);if(query.terms.some(term=>name.includes(term)))add("filename","Filename",name===query.text?100:name.startsWith(query.text)?90:containsTerms(name,query.terms)?75:45);
      if(query.terms.some(term=>normalize(photo.path).includes(term)))add("path","Folder / path",35);
      for(const tag of keywords)if(query.terms.some(term=>normalize(tag).includes(term)))add("tag",`Tag: ${tag}`,normalize(tag)===query.text?88:containsTerms(tag,query.terms)?65:45);
    }
    if(query.tag)add("tag",`Tag: ${query.tag}`,88);if(query.path)add("path",`Path: ${query.path}`,45);
    if(query.rating!==null||query.favorite)add("rating",rating===5?"Favorite · 5 stars":`${rating} stars`,55);
    if(query.type)add("type",type==="video"?"Video":"Photo",20);
    for(const item of current){
      if(item.kind==="person"&&(query.person?normalize(item.text).includes(query.person):query.terms.some(term=>normalize(item.text).includes(term))))add("person",`Person: ${item.text}`,query.person||containsTerms(item.text,query.terms)?88:60);
      if(item.kind==="ocr"&&query.terms.some(term=>normalize(item.text).includes(term)||item.matched_terms?.includes(term)))add("ocr",`Text: ${item.text.slice(0,160)}`,containsTerms([item.text,...item.matched_terms||[]].join(" "),query.terms)?68:50);
    }
    for(const item of semantic)add(item.kind,item.kind==="video_semantic"?"Video match":"Visual match",Math.min(72,Math.max(25,25+(item.score-.2)*100)),item.timestamp_seconds??undefined);
    // Keep the strongest evidence for each kind. Duplicate tags/faces cannot inflate relevance.
    const best=new Map<MatchKind,MatchReason>();for(const reason of reasons){const previous=best.get(reason.kind);if(!previous||previous.score<reason.score||previous.score===reason.score&&reason.label.localeCompare(previous.label)<0)best.set(reason.kind,reason);}
    const ordered=[...best.values()].sort((a,b)=>b.score-a.score||a.kind.localeCompare(b.kind)),primary=ordered[0]?.score||0;
    const score=Math.min(100,primary+Math.min(15,ordered.slice(1).reduce((total,r)=>total+r.score*.08,0)));
    results.push({media_id:id,root:parent,path:photo.path,media_type:type,score,reasons:ordered,matched_timestamps:[...new Set(semantic.flatMap(e=>e.timestamp_seconds===null?[]:[e.timestamp_seconds]))].sort((a,b)=>a-b)});
  }
  return results.sort((a,b)=>b.score-a.score||a.path.localeCompare(b.path)||a.root.localeCompare(b.root));
}

/** Carry a verified Live pair's motion evidence to its still identity without losing time. */
export function projectLiveEvidence(photos:SearchMedia[],root:string,pairs:Map<string,SearchMedia>|undefined,evidence:SearchEvidence[]):SearchEvidence[]{
 if(!pairs?.size)return evidence;const projected=[...evidence];for(const[imagePath,video]of pairs){const image=photos.find(p=>p.path===imagePath);if(!image)continue;for(const item of evidence){if(item.kind!=="video_semantic"||item.path!==video.path||item.root!==(video.libraryRoot||root)||item.modified_unix!==video.modified_unix)continue;projected.push({...item,root:image.libraryRoot||root,path:image.path,modified_unix:image.modified_unix,source_video_path:video.path});}}return projected;
}
