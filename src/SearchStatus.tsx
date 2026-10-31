export function SearchStatus({busy,readingTags,tagError,error,warnings,count}:{busy:boolean;readingTags:boolean;tagError:string;error:string;warnings:string[];count:number}){
 return <div className="search-status">
  {error?<p role="alert" className="field-error">{error}</p>:<p role="status">{busy?"Updating results…":`${count} matching ${count===1?"item":"items"}`}{readingTags?" · Reading tags…":""}</p>}
  {(warnings.length>0||tagError)&&<details><summary>Some search sources need attention</summary><ul>{[...warnings,...tagError?[tagError]:[]].map((warning,index)=><li key={index}>{warning}</li>)}</ul></details>}
 </div>;
}
