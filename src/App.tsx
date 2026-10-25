import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import PhotoMap, {type PhotoLocation} from "./PhotoMap";
import { Images, FolderOpen, Users, Tags, Settings2, Search, Moon, Sun, Monitor, ShieldCheck, ImageOff, HardDrive, ListFilter, ChevronRight, X, LayoutGrid, List, ZoomIn, ZoomOut, ArrowUpDown } from "lucide-react";

type View = "library" | "folders" | "collections" | "timeline" | "map" | "people" | "tags" | "settings";
type Collection = {id:string;workspaceId:string;name:string;paths:string[]};
type Workspace = { id: string; name: string; root: string; roots?: string[] };
type Theme = "light" | "dark" | "system";
type PhotoView = "grid" | "list";
type SortBy = "name" | "newest" | "oldest" | "largest" | "smallest";
type Photo = { path: string; name: string; extension: string; bytes: number; modified_unix: number; libraryRoot?: string };
type ScanResult = { photos: Photo[]; scanned_files: number; truncated: boolean };
const NAV: {id: View; title: string; icon: typeof Images}[] = [
  { id: "library", title: "Library", icon: Images },
  { id: "folders", title: "Folders", icon: FolderOpen },
  { id: "map", title: "Map", icon: Monitor },
  { id: "collections", title: "Collections", icon: Images },
  { id: "timeline", title: "Timeline", icon: List },
  { id: "people", title: "People", icon: Users },
  { id: "tags", title: "Tags", icon: Tags },
  { id: "settings", title: "Settings", icon: Settings2 }
];
const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export default function App() {
  const [view, setView] = useState<View>("library");
  const [workspaces, setWorkspaces] = useState<Workspace[]>(()=>{try{return JSON.parse(localStorage.getItem("openphoto.workspaces")||"[]") as Workspace[]}catch{return []}});
  const [activeWorkspace,setActiveWorkspace] = useState<string>(()=>localStorage.getItem("openphoto.activeWorkspace")||"");
  const [folderFilter,setFolderFilter] = useState("");
  const [collectionFilter,setCollectionFilter]=useState("");
  const [collections,setCollections]=useState<Collection[]>(()=>{try{return JSON.parse(localStorage.getItem("openphoto.collections")||"[]") as Collection[]}catch{return []}});
  useEffect(()=>localStorage.setItem("openphoto.collections",JSON.stringify(collections)),[collections]);
  const [geoPoints,setGeoPoints]=useState<PhotoLocation[]>([]);
  const [geoBusy,setGeoBusy]=useState(false);
  const [autoSync,setAutoSync]=useState(()=>localStorage.getItem("openphoto.autosync")==="true");
  const syncRunning=useRef<number|null>(null);
  const syncGeneration=useRef(0);
  useEffect(()=>{localStorage.setItem("openphoto.autosync",String(autoSync))},[autoSync]);
  const [lastSync,setLastSync]=useState("");
  const [syncChanges,setSyncChanges]=useState<{added:number;updated:number;removed:number;incomplete:boolean}|null>(null);
  const [duplicates, setDuplicates] = useState<{paths:string[];bytes:number}[]>([]);
  const [duplicateBusy, setDuplicateBusy] = useState(false);
  const [similarGroups,setSimilarGroups]=useState<{paths:string[];max_distance:number}[]>([]);
  const [mediaType,setMediaType] = useState<"all"|"photo"|"video">("all");
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("openphoto.theme") as Theme) || "system");
  const [root, setRoot] = useState("");
  useEffect(()=>{localStorage.setItem("openphoto.workspaces",JSON.stringify(workspaces))},[workspaces]);
  useEffect(()=>{localStorage.setItem("openphoto.activeWorkspace",activeWorkspace)},[activeWorkspace]);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [query, setQuery] = useState("");
  const [photoView, setPhotoView] = useState<PhotoView>("grid");
  const [sortBy, setSortBy] = useState<SortBy>("newest");
  const [tileSize, setTileSize] = useState(180);
  const [selection, setSelection] = useState<string[]>([]);
  const [tagText, setTagText] = useState("");
  const [applying, setApplying] = useState(false);
  const [tagMessage, setTagMessage] = useState("");
  const [scrollTop, setScrollTop] = useState(0);
  const [gridWidth, setGridWidth] = useState(940);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<Photo | null>(null);
  useEffect(() => {
    localStorage.setItem("openphoto.theme", theme);
    const body = document.documentElement;
    const update = () => body.dataset.theme = theme === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : theme;
    update();
    const media = matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [theme]);

  const exportWorkspace=()=>{
    const workspace=workspaces.find(w=>w.id===activeWorkspace);
    if(!workspace)return;
    const data={format:"open-photo-workspace",schemaVersion:1,exportedAt:new Date().toISOString(),workspace,collections:collections.filter(c=>c.workspaceId===activeWorkspace)};
    const blob=new Blob([JSON.stringify(data,null,2)],{type:"application/json"});
    const url=URL.createObjectURL(blob);
    const link=document.createElement("a");link.href=url;link.download="open-photo-workspace-backup.json";link.click();
    URL.revokeObjectURL(url);
  };
  const inspectDuplicates=async()=>{
    if(!result)return;
    setDuplicateBusy(true);setError("");
    try{
      const groups=await invoke<{paths:string[];bytes:number}[]>("find_workspace_duplicates",{roots,paths:result.photos.map(p=>p.path)});
      setDuplicates(groups);
    }catch(e){setError(String(e))}finally{setDuplicateBusy(false)}
  };
  const inspectSimilar=async()=>{
    if(!result)return;
    setDuplicateBusy(true);setError("");
    try{
      const matches=await invoke<{paths:string[];max_distance:number}[]>("find_similar_photos",{roots,paths:result.photos.map(p=>p.path)});
      setSimilarGroups(matches);
    }catch(e){setError(String(e))}finally{setDuplicateBusy(false)}
  };
  const workspace = workspaces.find(w=>w.id===activeWorkspace);
  const roots = workspace ? (workspace.roots?.length ? workspace.roots : workspace.root ? [workspace.root] : []) : [];
  const syncFolders = async (paths: string[]) => {
    if (!isDesktop || !paths.length || syncRunning.current===syncGeneration.current) return;
    const generation=syncGeneration.current;
    syncRunning.current=generation;
    setBusy(true); setError("");
    try {
      const scans = await Promise.allSettled(paths.map(async path=>{
        const response=await invoke<{scan:ScanResult;changes:{added:number;updated:number;removed:number;incomplete:boolean}}>("scan_indexed_with_changes",{root:path});
        const data=response.scan;
        return {...data,changes:response.changes,photos:data.photos.map(photo=>({...photo,libraryRoot:path}))};
      }));
      const success=scans.filter(s=>s.status==="fulfilled");
      const failures=scans.filter((s):s is PromiseRejectedResult=>s.status==="rejected");
      if (!success.length) throw Error(failures.map(x=>String(x.reason)).join("; "));
      const merged=new Map<string,Photo>();
      for(const scan of success)for(const photo of scan.value.photos)merged.set(photo.path,photo);
      if(generation!==syncGeneration.current)return;
      setResult({photos:[...merged.values()],scanned_files:success.reduce((n,s)=>n+s.value.scanned_files,0),truncated:success.some(s=>s.value.truncated)});
      if(failures.length)setError(failures.map(x=>String(x.reason)).join("; "));
      setSyncChanges(success.reduce<{added:number;updated:number;removed:number;incomplete:boolean}>((summary,entry)=>({added:summary.added+entry.value.changes.added,updated:summary.updated+entry.value.changes.updated,removed:summary.removed+entry.value.changes.removed,incomplete:summary.incomplete||entry.value.changes.incomplete}),{added:0,updated:0,removed:0,incomplete:false}));
      setLastSync(new Date().toLocaleString());setDuplicates([]);
    } catch(e){if(generation===syncGeneration.current)setError(String(e))}finally{if(syncRunning.current===generation)syncRunning.current=null;if(generation===syncGeneration.current)setBusy(false)}
  };
  const chooseFolder = async () => {
    if (!isDesktop) { setError("Folders must be selected in the Windows app."); return; }
    try {
      const path=await open({directory:true,multiple:false,title:"Add local media folder"});
      if(typeof path!=="string")return;
      const id=activeWorkspace||crypto.randomUUID();
      const current=workspaces.find(w=>w.id===id);
      const nextRoots=[...new Set([...(current?.roots?.length?current.roots:current?.root?[current.root]:[]),path])];
      setWorkspaces(old=>old.some(w=>w.id===id)?old.map(w=>w.id===id?{...w,root:nextRoots[0],roots:nextRoots}:w):[...old,{id,name:`Workspace ${old.length+1}`,root:path,roots:nextRoots}]);
      syncGeneration.current+=1;
      setActiveWorkspace(id);setRoot(nextRoots[0]);setSelection([]);setSelected(null);setFolderFilter("");
      await syncFolders(nextRoots);
    }catch(e){setError(String(e))}
  };
  useEffect(()=>{
    if(!autoSync||!roots.length)return;
    const timer=window.setInterval(()=>{if(syncRunning.current!==syncGeneration.current)void syncFolders(roots)},120000);
    return ()=>window.clearInterval(timer);
  },[autoSync,activeWorkspace,workspaces]);
  useEffect(()=>{
    if(!isDesktop)return;
    let disposed=false;
    let unlisten: (()=>void)|undefined;
    let debounce: number|undefined;
    const activeRoots=autoSync?roots:[];
    const setup=async()=>{
      try{
        await invoke("set_watched_folders",{roots:activeRoots});
        if(disposed||!autoSync)return;
        unlisten=await listen("openphoto-folder-changed",()=>{
          if(debounce!==undefined)window.clearTimeout(debounce);
          debounce=window.setTimeout(()=>{
            if(syncRunning.current!==syncGeneration.current)void syncFolders(activeRoots);
          },1500);
        });
        if(disposed)unlisten();
      }catch(error){if(!disposed)setError(String(error))}
    };
    void setup();
    return ()=>{
      disposed=true;
      unlisten?.();
      if(debounce!==undefined)window.clearTimeout(debounce);
    };
  },[autoSync,activeWorkspace,workspaces]);
  const openWorkspace=async(id:string)=>{
    const item=workspaces.find(w=>w.id===id);if(!item)return;
    syncGeneration.current+=1;
    setActiveWorkspace(id);setRoot(item.root);setSelection([]);setSelected(null);setResult(null);setFolderFilter("");
    const folders=item.roots?.length?item.roots:item.root?[item.root]:[];
    if(folders.length)await syncFolders(folders);
  };
  useEffect(()=>{
    if(view!=="map"||!result){setGeoPoints([]);return;}
    let active=true;
    setGeoBusy(true);
    const grouped=new Map<string,string[]>();
    for(const photo of result.photos) {
      if(!["jpg","jpeg","tif","tiff"].includes(photo.extension))continue;
      const parent=photo.libraryRoot||root;
      if(!parent)continue;
      const paths=grouped.get(parent)||[];
      if(paths.length<5000)paths.push(photo.path);
      grouped.set(parent,paths);
    }
    Promise.allSettled([...grouped].map(([parent,paths])=>invoke<PhotoLocation[]>("gps_for_photos",{root:parent,paths})))
      .then(scans=>{
        if(!active)return;
        setGeoPoints(scans.flatMap(scan=>scan.status==="fulfilled"?scan.value:[]));
        const errors=scans.filter((scan):scan is PromiseRejectedResult=>scan.status==="rejected");
        if(errors.length)setError(errors.map(scan=>String(scan.reason)).join("; "));
      }).finally(()=>{if(active)setGeoBusy(false)});
    return ()=>{active=false};
  },[root,result,view]);
  const folders=useMemo(()=>[...new Set((result?.photos||[]).map(p=>p.path.slice(0,Math.max(p.path.lastIndexOf("\\"),p.path.lastIndexOf("/"))).replace(p.libraryRoot||root,"").replace(/^[\\/]/,"")||"(root)"))].sort(),[result,root]);
  const livePhotoPairs=useMemo(()=>{
    const names=new Map<string,Set<string>>();
    for(const photo of result?.photos||[]){
      const normalized=photo.path.toLowerCase().replace(/\\/g,"/");
      const separator=normalized.lastIndexOf(".");
      if(separator<0)continue;
      const key=normalized.slice(0,separator);
      const extension=normalized.slice(separator+1);
      const types=names.get(key)||new Set<string>();
      types.add(extension);
      names.set(key,types);
    }
    return [...names.values()].filter(types=>types.has("mov")&&["heic","heif","jpg","jpeg"].some(type=>types.has(type))).length;
  },[result]);
  const currentCollection=collections.find(c=>c.id===collectionFilter&&c.workspaceId===activeWorkspace);
  const visible = useMemo(() => {
    const photos = result?.photos.filter(p => p.name.toLowerCase().includes(query.toLowerCase()) && (!folderFilter || p.path.includes(folderFilter)) && (!currentCollection || currentCollection.paths.includes(p.path)) && (mediaType==="all" || (mediaType==="video" ? ["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension) : !["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension)))) || [];
    return photos.sort((a,b) => {
      switch (sortBy) {
        case "name": return a.name.localeCompare(b.name) || a.path.localeCompare(b.path);
        case "newest": return b.modified_unix - a.modified_unix || a.path.localeCompare(b.path);
        case "oldest": return a.modified_unix - b.modified_unix || a.path.localeCompare(b.path);
        case "largest": return b.bytes - a.bytes || a.path.localeCompare(b.path);
        case "smallest": return a.bytes - b.bytes || a.path.localeCompare(b.path);
      }
    });
  }, [result, query, sortBy, folderFilter, mediaType, currentCollection]);
  useEffect(() => setScrollTop(0), [query, sortBy, photoView, tileSize, root, folderFilter, mediaType]);
  const gridGap = 14;
  const columns = Math.max(1, Math.floor((gridWidth - 48 + gridGap) / (tileSize + gridGap)));
  const rowHeight = Math.max(tileSize + 52, (gridWidth - 48 - (columns - 1) * gridGap) / columns + 43);
  const windowStart = Math.max(0, Math.floor(scrollTop / rowHeight) - 2);
  const rowCount = Math.ceil(visible.length / columns);
  const windowEnd = Math.min(rowCount, windowStart + Math.ceil(700 / rowHeight) + 6);
  const windowPhotos = visible.slice(windowStart * columns, windowEnd * columns);
  const toggleSelection = (path: string) => setSelection(old => old.includes(path) ? old.filter(p => p !== path) : [...old, path]);
  const applyTags = async () => {
    if (!root || !selection.length || !tagText.trim()) return;
    const tags = [...new Set(tagText.split(",").map(x=>x.trim()).filter(Boolean))];
    if (!tags.length) return;
    if (!window.confirm(`Write NEW XMP sidecars containing ${tags.join(", ")} for ${selection.length} photos? Existing XMP sidecars will be skipped and originals never modified.`)) return;
    setApplying(true); setTagMessage("");
    try {
      const groups=new Map<string,string[]>();for(const path of selection){const parent=result?.photos.find(p=>p.path===path)?.libraryRoot||root;groups.set(parent,[...(groups.get(parent)||[]),path]);}
      const outcomes=await Promise.all([...groups].map(([parent,paths])=>invoke<{created:number;skipped:number;errors:string[]}>("bulk_create_tags",{root:parent,paths,tags})));
      const r={created:outcomes.reduce((n,x)=>n+x.created,0),skipped:outcomes.reduce((n,x)=>n+x.skipped,0),errors:outcomes.flatMap(x=>x.errors)};
      setTagMessage(`Created ${r.created} sidecars; skipped ${r.skipped}. ${r.errors.length ? r.errors.slice(0,3).join("; ") : ""}`);
      if (!r.errors.length) setSelection([]);
    } catch(e) { setTagMessage(String(e)); } finally {setApplying(false);}
  };
  const bytes = result?.photos.reduce((n, p) => n + p.bytes, 0) || 0;
  const title = NAV.find(x => x.id === view)?.title || "Library";
  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Images size={23}/></div><div><strong>Open-Photo</strong><small>YOUR PHOTO LIBRARY</small></div></div>
      <div className="sidebar-label">WORKSPACE</div>
      <select className="workspace-select" aria-label="Active workspace" value={activeWorkspace} onChange={e=>openWorkspace(e.target.value)}><option value="">Select a workspace</option>{workspaces.map(w=><option key={w.id} value={w.id}>{w.name}</option>)}</select>
      <button className="workspace-create" onClick={()=>{const name=window.prompt("Name this workspace");if(!name?.trim())return;const id=crypto.randomUUID();setWorkspaces(old=>[...old,{id,name:name.trim(),root:""}]);setActiveWorkspace(id);setRoot("");setResult(null);setSelection([]);setView("library")}}>+ New workspace</button>
      <nav aria-label="Main navigation">{NAV.map(({ id, title, icon: Icon }) => <button key={id} className={view === id ? "nav-button active" : "nav-button"} onClick={() => setView(id)} aria-current={view === id ? "page" : undefined}><Icon size={18}/>{title}{view === id && <ChevronRight size={15} className="nav-chevron"/>}</button>)}</nav>
      <div className="sidebar-spacer"/>

      <div className="sidebar-footer">Open-Photo <span>v0.1.0 preview</span></div>
    </aside>

    <div className="main">
      <header className="topbar"><div className="breadcrumb">Workspace <ChevronRight size={14}/> <strong>{title}</strong></div><div className="top-actions"><span className="readonly-chip"><ShieldCheck size={14}/> READ-ONLY</span><button className="icon-button" onClick={() => setTheme(theme === "dark" ? "light" : "dark")} title="Toggle light and dark mode" aria-label="Toggle color theme">{theme === "dark" ? <Sun size={19}/> : <Moon size={19}/>}</button></div></header>
      <main className="page">
        {view === "library" && <>
          <div className="page-heading"><div><span className="eyebrow">YOUR COLLECTION</span><h1>Photo Library</h1><p>Browse your memories privately, right from your computer.</p></div><button className="primary-button" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/>{busy ? "Scanning..." : root ? "Change Folder" : "Open Folder"}</button></div>
          <div className="sync-toolbar"><strong>{roots.length} folders</strong><button className="secondary-button" onClick={chooseFolder}>+ Add folder</button><button className="secondary-button" disabled={!roots.length||busy} onClick={()=>syncFolders(roots)}>{busy?"Scanning…":"Sync Now"}</button><label><input type="checkbox" checked={autoSync} onChange={e=>setAutoSync(e.target.checked)}/> Auto-sync every 2 minutes</label><small>{lastSync&&`Last indexed: ${lastSync}`}</small>{syncChanges&&<small>{`Changes: +${syncChanges.added} added, ${syncChanges.updated} updated, ${syncChanges.removed} removed${syncChanges.incomplete?" (partial scan)":""}`}</small>}{roots.map(p=><span key={p} className="folder-chip" title={p}>{p}<button aria-label={`Remove ${p}`} onClick={()=>{const remaining=roots.filter(x=>x!==p);setWorkspaces(old=>old.map(w=>w.id===activeWorkspace?{...w,roots:remaining,root:remaining[0]||""}:w));setRoot(remaining[0]||"");setResult(null);if(remaining.length)void syncFolders(remaining)}}>×</button></span>)}</div><div className="stats"><div className="stat"><div className="stat-icon"><Images size={20}/></div><div><span>Indexed photos</span><strong>{result?.photos.length.toLocaleString() ?? "—"}</strong></div></div><div className="stat"><div className="stat-icon"><HardDrive size={20}/></div><div><span>Selected files size</span><strong>{result ? formatBytes(bytes) : "—"}</strong></div></div><div className="stat"><div className="stat-icon"><ShieldCheck size={20}/></div><div><span>File protection</span><strong>Read-only</strong></div></div></div>
          <section className="library-panel">
            <div className="panel-toolbar"><div><h2>All Photos</h2>{folderFilter&&<button className="secondary-button" onClick={()=>setFolderFilter("")}>Clear folder: {folderFilter}</button>}<p>{root ? root : "Select a folder to begin browsing"}</p></div><div className="search-box"><Search size={17}/><input aria-label="Filter photos by filename" placeholder="Search filenames..." value={query} onChange={e => setQuery(e.target.value)} disabled={!result}/><ListFilter size={16}/></div></div>
            <div className="view-toolbar">
              <div className="view-switch" role="group" aria-label="Photo view">
                <button className={photoView === "grid" ? "active" : ""} title="Icon grid" aria-label="Icon grid" aria-pressed={photoView === "grid"} onClick={()=>setPhotoView("grid")}><LayoutGrid size={18}/></button>
                <button className={photoView === "list" ? "active" : ""} title="List view" aria-label="List view" aria-pressed={photoView === "list"} onClick={()=>setPhotoView("list")}><List size={18}/></button>
              </div>
              <label className="sort-control"><ArrowUpDown size={16}/><span>Sort:</span><select value={sortBy} aria-label="Sort photos" onChange={e=>setSortBy(e.target.value as SortBy)}><option value="newest">Newest first</option><option value="oldest">Oldest first</option><option value="name">Name A–Z</option><option value="largest">Largest first</option><option value="smallest">Smallest first</option></select></label>
              {photoView === "grid" && <div className="zoom-control"><ZoomOut size={17}/><input aria-label="Thumbnail size" type="range" min="110" max="300" step="10" value={tileSize} onChange={e=>setTileSize(Number(e.target.value))}/><ZoomIn size={17}/><span>{tileSize}px</span></div>}
            </div>
            {result && <div className="selection-toolbar">{collectionFilter&&<button className="secondary-button" onClick={()=>setCollectionFilter("")}>Show all (clear collection)</button>}<label className="sort-control">Media <select aria-label="Media type" value={mediaType} onChange={e=>setMediaType(e.target.value as "all"|"photo"|"video")}><option value="all">All media</option><option value="photo">Photos</option><option value="video">Videos</option></select></label><span>{selection.length} selected</span><button className="secondary-button" onClick={()=>setSelection(visible.map(p=>p.path))}>Select matching</button><button className="secondary-button" onClick={()=>setSelection([])}>Clear selection</button><input aria-label="New XMP tags (comma separated)" placeholder="Tags separated by commas" value={tagText} onChange={e=>setTagText(e.target.value)}/><button className="primary-button" disabled={!selection.length || !tagText.trim() || applying} onClick={applyTags}>{applying?"Writing…":"Add tags to new sidecars"}</button></div>}
            {result && livePhotoPairs>0 && <p className="results-note">{livePhotoPairs} potential iPhone Live Photo pairs found (same folder and filename stem). Original image and video files remain separate.</p>}
            {result && <div className="duplicate-bar"><button className="secondary-button" disabled={duplicateBusy} onClick={inspectDuplicates}>Find Exact Duplicates</button><button className="secondary-button" disabled={duplicateBusy} onClick={inspectSimilar}>Find Similar Photos</button>{duplicateBusy&&<span>Analyzing local files…</span>}{duplicates.length>0&&<span>{duplicates.length} identical-file groups</span>}{similarGroups.length>0&&<span>{similarGroups.length} visually similar groups</span>}{[...duplicates.map(group=>({paths:group.paths,label:"Identical files"})),...similarGroups.map(group=>({paths:group.paths,label:"Visually similar photos"}))].map((group,i)=><div className="duplicate-group" key={i}><strong>{group.label}</strong>{group.paths.map(path=><small key={path}>{path}</small>)}</div>)}</div>}
            {tagMessage && <div className="results-note" role="status">{tagMessage}</div>}
            {error && <div role="alert" className="error">{error}</div>}
            {busy && <div className="empty"><div className="empty-symbol"><Images size={28}/></div><h3>Scanning your folder</h3><p>Reading file names and sizes. No photos are being changed.</p></div>}
            {!busy && !result && <div className="empty"><div className="empty-symbol"><ImageOff size={32}/></div><h3>Your library starts here</h3><p>Choose a folder on your computer to discover supported photo files. Images display as thumbnails where supported, with local-only processing.</p><button className="secondary-button" onClick={chooseFolder}><FolderOpen size={16}/> Select Photo Folder</button></div>}
            {!busy && result && <><div className="results-note">{visible.length} matching files · {result.scanned_files.toLocaleString()} filesystem entries scanned {result.truncated ? "· scan limit reached" : ""}</div><div className={photoView === "grid" ? "photo-grid virtual-photo-grid" : "file-list"} onScroll={e=>setScrollTop(e.currentTarget.scrollTop)} ref={el=>{if(el && Math.abs(el.clientWidth-gridWidth)>5)setGridWidth(el.clientWidth);}} style={photoView === "grid" ? { display:"block", padding:0 } : undefined}>
              {photoView === "grid" ? <div style={{height:rowCount*rowHeight,position:"relative"}}><div className="virtual-rows" style={{position:"absolute",top:windowStart*rowHeight,left:24,right:24,display:"grid",gridTemplateColumns:`repeat(${columns},minmax(0,1fr))`,gridAutoRows:rowHeight,gap:0}}>
                {windowPhotos.map(p=><div className="tile-cell" key={p.path}><button className={selection.includes(p.path) ? "photo-tile is-selected" : "photo-tile"} onClick={e=>e.ctrlKey||e.metaKey ? toggleSelection(p.path):setSelected(p)}><PhotoThumb root={p.libraryRoot||root} photo={p} maxEdge={Math.max(360,tileSize*2)} /><span className="tile-name" title={p.name}>{p.name}</span></button><input type="checkbox" aria-label={`Select ${p.name}`} checked={selection.includes(p.path)} onChange={()=>toggleSelection(p.path)}/></div>)}
              </div></div> : visible.map(p=><div key={p.path} className="selectable-row"><input type="checkbox" aria-label={`Select ${p.name}`} checked={selection.includes(p.path)} onChange={()=>toggleSelection(p.path)}/><button className="file-row" onClick={()=>setSelected(p)}><div className="file-avatar"><PhotoThumb root={root} photo={p} maxEdge={96}/></div><div className="file-name"><strong>{p.name}</strong><small>{p.extension.toUpperCase()} · {formatBytes(p.bytes)}</small></div><ChevronRight size={16}/></button></div>)}
            </div>{visible.length === 0 && <div className="empty"><p>No supported photos matched this search.</p></div>}</>}
          </section>
        </>}
        {view === "folders" && <><span className="eyebrow">LIBRARY ORGANIZATION</span><h1>Folders</h1><p>Browse the folder structure from your selected workspace.</p><div className="folder-list">{folders.map(folder=><button key={folder} className="folder-item" onClick={()=>{setFolderFilter(folder==="(root)"?"":folder);setView("library")}}><FolderOpen size={20}/>{folder}<ChevronRight size={16}/></button>)}{folders.length===0&&<p>Open a workspace folder to browse its subfolders.</p>}</div></>}
        {view === "map" && <><span className="eyebrow">PHOTO LOCATIONS</span><h1>Map</h1><p>Zoom and explore GPS-tagged photos from your current workspace. Select a marker to preview its photo.</p>{geoBusy?<p>Reading embedded GPS locations…</p>:<p>{geoPoints.length} photo locations found. Currently supports GPS from JPEG and TIFF; HEIC/MOV location metadata is planned.</p>}{root&&result?<PhotoMap points={geoPoints} onChoose={path=>{const photo=result.photos.find(p=>p.path===path);if(photo)setSelected(photo)}}/>:<div className="placeholder-panel"><p>Open a workspace and scan a photo folder first.</p></div>}</>}
        {view === "collections" && <><span className="eyebrow">VIRTUAL ORGANIZATION</span><h1>Collections</h1><p>Group media without moving original files.</p><button className="primary-button" onClick={()=>{const name=window.prompt("Collection name");if(name?.trim())setCollections(old=>[...old,{id:crypto.randomUUID(),workspaceId:activeWorkspace,name:name.trim(),paths:[...selection]}])}}>Create collection {selection.length? `with ${selection.length} selected` : ""}</button><div className="folder-list">{collections.filter(c=>c.workspaceId===activeWorkspace).map(c=><div key={c.id} className="collection-row"><button className="folder-item" onClick={()=>{setCollectionFilter(c.id);setView("library")}}><Images size={18}/>{c.name} · {c.paths.length} items</button><button className="secondary-button" onClick={()=>setCollections(old=>old.filter(x=>x.id!==c.id))}>Remove</button></div>)}</div></>}
        {view === "timeline" && <><span className="eyebrow">BY MODIFIED DATE</span><h1>Timeline</h1><p>Browse files by modification month. Capture-date sorting will follow when EXIF and QuickTime date extraction is complete.</p><div className="folder-list">{[...new Set((result?.photos||[]).map(p=>new Date(p.modified_unix*1000).toLocaleDateString(undefined,{year:"numeric",month:"long"})))].map(month=><button className="folder-item" key={month} onClick={()=>{setQuery("");setSortBy("newest");setView("library")}}>{month}</button>)}</div></>}
        {view === "people" && <Placeholder title="People" subtitle="Organize familiar faces without giving up control of your data." icon={Users} description="Coming in a later milestone: local, opt-in face detection, human-reviewed identities, and portable XMP regions."/>}
        {view === "tags" && <Placeholder title="Tags" subtitle="Give every memory context that travels with the file." icon={Tags} description="Coming in a later milestone: hierarchical tags and verified, reversible XMP sidecar editing."/>}
        {view === "settings" && <div className="settings-view"><span className="eyebrow">PREFERENCES</span><h1>Settings</h1><p>Personalize the appearance of your workspace.</p><div className="settings-card"><div><h2>Appearance</h2><p>Choose your preferred color theme.</p></div><div className="theme-options">{([{id:"system",label:"System",Icon:Monitor},{id:"light",label:"Light",Icon:Sun},{id:"dark",label:"Dark",Icon:Moon}] as const).map(({id,label,Icon})=><button key={id} className={theme === id ? "theme-option selected" : "theme-option"} onClick={()=>setTheme(id)}><Icon size={17}/>{label}</button>)}</div></div><div className="settings-card"><div><h2>Workspace backup</h2><p>Export selected workspace settings, folder references and virtual collections to JSON. Photo and video files are not copied.</p></div><button className="secondary-button" disabled={!activeWorkspace} onClick={exportWorkspace}>Export Workspace JSON</button></div><div className="settings-card"><div><h2>File safety</h2><p>Open-Photo currently indexes paths, sizes, and modification times only. File writes and deletions are not implemented.</p></div><span className="safety-pill">Protected</span></div></div>}
      </main>
    </div>
    {selected && <div className="modal-backdrop" role="presentation" onClick={()=>setSelected(null)}><section className="details" role="dialog" aria-modal="true" aria-label="Photo details" onClick={e=>e.stopPropagation()}><div className="details-header"><h2>File Details</h2><button className="icon-button" onClick={()=>setSelected(null)} aria-label="Close details"><X size={20}/></button></div><div className="detail-preview">{["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)?<div className="photo-fallback"><Images size={42}/><span>Embedded playback is planned. Open this video using your default Windows player.</span></div>:<PhotoThumb root={selected.libraryRoot||root} photo={selected} maxEdge={1600}/>}</div>{["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)&&<button className="primary-button" onClick={()=>invoke("open_video",{root:selected.libraryRoot||root,path:selected.path}).catch(e=>setError(String(e)))}>Open in Windows player</button>}<dl><dt>Name</dt><dd>{selected.name}</dd><dt>Location</dt><dd className="break-anywhere">{selected.path}</dd><dt>File size</dt><dd>{formatBytes(selected.bytes)}</dd><dt>File format</dt><dd>{selected.extension.toUpperCase()}</dd><dt>Modified</dt><dd>{selected.modified_unix ? new Date(selected.modified_unix*1000).toLocaleString() : "Unknown"}</dd></dl><div className="details-foot"><ShieldCheck size={16}/> File opened read-only</div></section></div>}
  </div>;
}

function Placeholder({title,subtitle,icon:Icon,description}:{title:string;subtitle:string;icon:typeof Images;description:string}) {
  return <><span className="eyebrow">COMING SOON</span><h1>{title}</h1><p>{subtitle}</p><div className="placeholder-panel"><div className="empty-symbol"><Icon size={30}/></div><h2>Built for the long term</h2><p>{description}</p></div></>;
}
function formatBytes(n:number){if(n<1024)return n+" B";const u=["KB","MB","GB","TB"];let i=-1;do{n/=1024;i++}while(n>=1024&&i<u.length-1);return n.toFixed(n<10?1:0)+" "+u[i];}

function PhotoThumb({root,photo,maxEdge}:{root:string;photo:Photo;maxEdge:number}) {
  const [src,setSrc] = useState("");
  const [failed,setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    setSrc(""); setFailed(false);
    if (!isDesktop || !root) return;
    invoke<string>("preview_image",{root,path:photo.path,maxEdge})
      .then(data=>{if(active)setSrc(data);})
      .catch(()=>{if(active)setFailed(true);});
    return ()=>{active=false;};
  },[root,photo.path,maxEdge]);
  return src ? <img className="photo-image" src={src} alt={photo.name} loading="lazy"/> : <div className="photo-fallback"><Images size={28}/><small>{failed ? "Preview unavailable" : "Loading preview…"}</small></div>;
}
