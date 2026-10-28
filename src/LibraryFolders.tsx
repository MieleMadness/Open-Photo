export function LibraryFolders({roots,onRemove}:{roots:string[];onRemove:(path:string)=>void}){
  return <section className="library-folders" aria-label="Library folders">
    <h2>Folders in this library</h2>
    <p>Remove a folder from this view without deleting photos, videos, or saved edits. You can add it again later.</p>
    {roots.length?<ul>{roots.map(path=><li key={path}><span title={path}>{path}</span><button className="secondary-button" aria-label={`Remove ${path} from library`} onClick={()=>onRemove(path)}>Remove from library</button></li>)}</ul>:<p>No folders added. Choose Add folder to start.</p>}
  </section>;
}
