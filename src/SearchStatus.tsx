export function SearchStatus({busy,readingTags,tagError,error,warnings,aiEnabled,submitted,count}:{busy:boolean;readingTags:boolean;tagError:string;error:string;warnings:string[];aiEnabled:boolean;submitted:boolean;count:number}){
 return <div className="search-status">
  {error?<p role="alert" className="field-error">{error}</p>:<p role="status">{count} matching {count===1?"item":"items"}{busy?" · Searching local indexes…":""}{readingTags?" · Reading tags…":""}</p>}
  <p className="search-help">{aiEnabled?(submitted?"Indexed text, named People and image content are included.":"Indexed text and named People are included. Press Search or Enter to include image content."):"Turn on local AI in Settings to include indexed text, named People and image content."}</p>
  <details><summary>Search tips</summary><p>Search names, paths and managed XMP tags. Filters: <code>is:favorite</code>, <code>rating:&gt;=4</code>, <code>type:video</code>, <code>tag:family</code>, <code>person:"Alex Lee"</code>. Put quotes around names or values containing spaces.</p></details>
  {(warnings.length>0||tagError)&&<details><summary>Some search sources need attention</summary><ul>{[...warnings,...tagError?[tagError]:[]].map((warning,index)=><li key={index}>{warning}</li>)}</ul></details>}
 </div>;
}
