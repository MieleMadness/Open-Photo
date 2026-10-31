import {useEffect,useState} from "react";
import {ChevronRight,Images,Layers3,Moon,PanelLeftClose,PanelLeftOpen,Sun,type LucideIcon} from "lucide-react";
import type {Workspace} from "./libraryWorkspace";
import {saveSetting} from "./userData";

type Props<T extends string>={items:{id:T;title:string;icon:LucideIcon}[];view:T;onNavigate:(view:T)=>void;workspaces:Workspace[];workspace:string;onWorkspace:(id:string)=>void;onCreateWorkspace:()=>void;theme:string;onTheme:(theme:"light"|"dark")=>void;version:string};
/** Width is an explicit app preference. Compact rows never stretch with the window height. */
export function AppSidebar<T extends string>({items,view,onNavigate,workspaces,workspace,onWorkspace,onCreateWorkspace,theme,onTheme,version}:Props<T>){
  const [collapsed,setCollapsed]=useState(()=>localStorage.getItem("openphoto.sidebarCollapsed")==="true");
  useEffect(()=>saveSetting("openphoto.sidebarCollapsed",String(collapsed)),[collapsed]);
  const workspaceName=workspaces.find(w=>w.id===workspace)?.name||"Select a workspace";
  return <aside className="sidebar" data-collapsed={collapsed} aria-label="Application sidebar">
    <div className="sidebar-header"><div className="brand" aria-label="Open-Photo"><div className="brand-mark"><Images size={23}/></div><div className="brand-copy"><strong>Open-Photo</strong><small>YOUR PHOTO LIBRARY</small></div></div>
      <button className="icon-button sidebar-toggle" aria-label={collapsed?"Expand menu":"Collapse menu"} title={collapsed?"Expand menu":"Collapse menu"} aria-expanded={!collapsed} aria-controls="app-navigation" onClick={()=>setCollapsed(value=>!value)}>{collapsed?<PanelLeftOpen size={20}/>:<PanelLeftClose size={20}/>}</button>
    </div>
    <div className="sidebar-workspace" title={`Workspace: ${workspaceName}`}><label><span className="workspace-label">Workspace</span><span className="workspace-picker"><Layers3 className="workspace-picker-icon" size={20} aria-hidden="true"/><select className="workspace-select" aria-label="Active workspace" value={workspace} onChange={event=>event.target.value==="__new__"?onCreateWorkspace():onWorkspace(event.target.value)}><option value="">Select a workspace</option>{workspaces.map(w=><option key={w.id} value={w.id}>{w.name}</option>)}<option value="__new__">+ Add new workspace…</option></select></span></label></div>
    <nav id="app-navigation" className="sidebar-nav" aria-label="Main navigation">{items.map(({id,title,icon:Icon})=><button key={id} className={view===id?"nav-button active":"nav-button"} onClick={()=>onNavigate(id)} title={title} aria-label={title} aria-current={view===id?"page":undefined}><Icon size={18}/><span className="nav-label">{title}</span>{view===id&&<ChevronRight size={16} className="nav-chevron"/>}</button>)}</nav>
    <div className="sidebar-theme" aria-label="Appearance"><span className="sidebar-label">APPEARANCE</span><div className="theme-options sidebar-theme-options">{([{id:"light",label:"Light",Icon:Sun},{id:"dark",label:"Dark",Icon:Moon}] as const).map(({id,label,Icon})=><button key={id} className={theme===id?"theme-option selected":"theme-option"} onClick={()=>onTheme(id)} title={label} aria-label={label} aria-pressed={theme===id}><Icon size={16}/><span className="theme-label">{label}</span></button>)}</div></div>
    <div className="sidebar-footer">Open-Photo <span>{version?`v${version} preview`:"Development preview"}</span></div>
  </aside>;
}
