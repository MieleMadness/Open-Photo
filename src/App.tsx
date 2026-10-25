import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Images, FolderOpen, Users, Tags, Settings2, Search, Moon, Sun, Monitor, ShieldCheck, ImageOff, HardDrive, ListFilter, ChevronRight, X } from "lucide-react";

type View = "library" | "people" | "tags" | "settings";
type Theme = "light" | "dark" | "system";
type Photo = { path: string; name: string; extension: string; bytes: number; modified_unix: number };
type ScanResult = { photos: Photo[]; scanned_files: number; truncated: boolean };
const NAV: {id: View; title: string; icon: typeof Images}[] = [
  { id: "library", title: "Library", icon: Images },
  { id: "people", title: "People", icon: Users },
  { id: "tags", title: "Tags", icon: Tags },
  { id: "settings", title: "Settings", icon: Settings2 }
];
const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export default function App() {
  const [view, setView] = useState<View>("library");
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("openphoto.theme") as Theme) || "system");
  const [root, setRoot] = useState("");
  const [result, setResult] = useState<ScanResult | null>(null);
  const [query, setQuery] = useState("");
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

  const chooseFolder = async () => {
    if (!isDesktop) { setError("Choose folders inside the installed Windows application, not the web preview."); return; }
    setError("");
    try {
      const path = await open({ directory: true, multiple: false, title: "Select a photo library (read-only scan)" });
      if (typeof path !== "string") return;
      setRoot(path); setBusy(true); setResult(null); setSelected(null);
      const items = await invoke<ScanResult>("scan_library", { root: path });
      setResult(items);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const visible = useMemo(() => result?.photos.filter(p => p.name.toLowerCase().includes(query.toLowerCase())) || [], [result, query]);
  const bytes = result?.photos.reduce((n, p) => n + p.bytes, 0) || 0;
  const title = NAV.find(x => x.id === view)?.title || "Library";
  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Images size={23}/></div><div><strong>Open-Photo</strong><small>YOUR PHOTO LIBRARY</small></div></div>
      <div className="sidebar-label">WORKSPACE</div>
      <nav aria-label="Main navigation">{NAV.map(({ id, title, icon: Icon }) => <button key={id} className={view === id ? "nav-button active" : "nav-button"} onClick={() => setView(id)} aria-current={view === id ? "page" : undefined}><Icon size={18}/>{title}{view === id && <ChevronRight size={15} className="nav-chevron"/>}</button>)}</nav>
      <div className="sidebar-spacer"/>
      <div className="privacy-note"><ShieldCheck size={17}/><div><strong>Private by default</strong><span>No cloud upload. Original files stay untouched.</span></div></div>
      <div className="sidebar-footer">Open-Photo <span>v0.1.0 preview</span></div>
    </aside>

    <div className="main">
      <header className="topbar"><div className="breadcrumb">Workspace <ChevronRight size={14}/> <strong>{title}</strong></div><div className="top-actions"><span className="readonly-chip"><ShieldCheck size={14}/> READ-ONLY</span><button className="icon-button" onClick={() => setTheme(theme === "dark" ? "light" : "dark")} title="Toggle light and dark mode" aria-label="Toggle color theme">{theme === "dark" ? <Sun size={19}/> : <Moon size={19}/>}</button></div></header>
      <main className="page">
        {view === "library" && <>
          <div className="page-heading"><div><span className="eyebrow">YOUR COLLECTION</span><h1>Photo Library</h1><p>Browse your memories privately, right from your computer.</p></div><button className="primary-button" onClick={chooseFolder} disabled={busy}><FolderOpen size={18}/>{busy ? "Scanning..." : root ? "Change Folder" : "Open Folder"}</button></div>
          <div className="stats"><div className="stat"><div className="stat-icon"><Images size={20}/></div><div><span>Indexed photos</span><strong>{result?.photos.length.toLocaleString() ?? "—"}</strong></div></div><div className="stat"><div className="stat-icon"><HardDrive size={20}/></div><div><span>Selected files size</span><strong>{result ? formatBytes(bytes) : "—"}</strong></div></div><div className="stat"><div className="stat-icon"><ShieldCheck size={20}/></div><div><span>File protection</span><strong>Read-only</strong></div></div></div>
          <section className="library-panel">
            <div className="panel-toolbar"><div><h2>All Photos</h2><p>{root ? root : "Select a folder to begin browsing"}</p></div><div className="search-box"><Search size={17}/><input aria-label="Filter photos by filename" placeholder="Search filenames..." value={query} onChange={e => setQuery(e.target.value)} disabled={!result}/><ListFilter size={16}/></div></div>
            {error && <div role="alert" className="error">{error}</div>}
            {busy && <div className="empty"><div className="empty-symbol"><Images size={28}/></div><h3>Scanning your folder</h3><p>Reading file names and sizes. No photos are being changed.</p></div>}
            {!busy && !result && <div className="empty"><div className="empty-symbol"><ImageOff size={32}/></div><h3>Your library starts here</h3><p>Choose a folder on your computer to discover supported photo files. This first milestone lists files; image thumbnails come next.</p><button className="secondary-button" onClick={chooseFolder}><FolderOpen size={16}/> Select Photo Folder</button></div>}
            {!busy && result && <><div className="results-note">{visible.length} matching files · {result.scanned_files.toLocaleString()} filesystem entries scanned {result.truncated ? "· scan limit reached" : ""}</div><div className="file-list">{visible.slice(0,500).map(p => <button key={p.path} className="file-row" onClick={() => setSelected(p)}><div className="file-avatar"><Images size={19}/></div><div className="file-name"><strong>{p.name}</strong><small>{p.extension.toUpperCase()} · {formatBytes(p.bytes)}</small></div><ChevronRight size={16}/></button>)}</div>{visible.length > 500 && <div className="results-note">Displaying first 500 matches. Narrow your search to find more.</div>}{visible.length === 0 && <div className="empty"><p>No supported photos matched this search.</p></div>}</>}
          </section>
        </>}
        {view === "people" && <Placeholder title="People" subtitle="Organize familiar faces without giving up control of your data." icon={Users} description="Coming in a later milestone: local, opt-in face detection, human-reviewed identities, and portable XMP regions."/>}
        {view === "tags" && <Placeholder title="Tags" subtitle="Give every memory context that travels with the file." icon={Tags} description="Coming in a later milestone: hierarchical tags and verified, reversible XMP sidecar editing."/>}
        {view === "settings" && <div className="settings-view"><span className="eyebrow">PREFERENCES</span><h1>Settings</h1><p>Personalize the appearance of your workspace.</p><div className="settings-card"><div><h2>Appearance</h2><p>Choose your preferred color theme.</p></div><div className="theme-options">{([{id:"system",label:"System",Icon:Monitor},{id:"light",label:"Light",Icon:Sun},{id:"dark",label:"Dark",Icon:Moon}] as const).map(({id,label,Icon})=><button key={id} className={theme === id ? "theme-option selected" : "theme-option"} onClick={()=>setTheme(id)}><Icon size={17}/>{label}</button>)}</div></div><div className="settings-card"><div><h2>File safety</h2><p>Open-Photo currently indexes paths, sizes, and modification times only. File writes and deletions are not implemented.</p></div><span className="safety-pill">Protected</span></div></div>}
      </main>
    </div>
    {selected && <div className="modal-backdrop" role="presentation" onClick={()=>setSelected(null)}><section className="details" role="dialog" aria-modal="true" aria-label="Photo details" onClick={e=>e.stopPropagation()}><div className="details-header"><h2>File Details</h2><button className="icon-button" onClick={()=>setSelected(null)} aria-label="Close details"><X size={20}/></button></div><div className="detail-preview"><Images size={42}/><span>Thumbnail preview coming soon</span></div><dl><dt>Name</dt><dd>{selected.name}</dd><dt>Location</dt><dd className="break-anywhere">{selected.path}</dd><dt>File size</dt><dd>{formatBytes(selected.bytes)}</dd><dt>File format</dt><dd>{selected.extension.toUpperCase()}</dd><dt>Modified</dt><dd>{selected.modified_unix ? new Date(selected.modified_unix*1000).toLocaleString() : "Unknown"}</dd></dl><div className="details-foot"><ShieldCheck size={16}/> File opened read-only</div></section></div>}
  </div>;
}

function Placeholder({title,subtitle,icon:Icon,description}:{title:string;subtitle:string;icon:typeof Images;description:string}) {
  return <><span className="eyebrow">COMING SOON</span><h1>{title}</h1><p>{subtitle}</p><div className="placeholder-panel"><div className="empty-symbol"><Icon size={30}/></div><h2>Built for the long term</h2><p>{description}</p></div></>;
}
function formatBytes(n:number){if(n<1024)return n+" B";const u=["KB","MB","GB","TB"];let i=-1;do{n/=1024;i++}while(n>=1024&&i<u.length-1);return n.toFixed(n<10?1:0)+" "+u[i];}
