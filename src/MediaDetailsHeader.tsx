import {ChevronLeft,ChevronRight,X} from "lucide-react";
type Props={filmstripVisible?:boolean;onToggleFilmstrip?:()=>void;video:boolean;editing:boolean;fullscreen:boolean;canPrevious:boolean;canNext:boolean;onEdit:()=>void;onFullscreen:()=>void;onPrevious:()=>void;onNext:()=>void;onClose:()=>void};
/** File identity lives in the summary below, leaving this toolbar room for the actual actions. */
export function MediaDetailsHeader({filmstripVisible,onToggleFilmstrip,video,editing,fullscreen,canPrevious,canNext,onEdit,onFullscreen,onPrevious,onNext,onClose}:Props){
  return <div className="details-header"><h2 className="details-title">File Details</h2><div className="details-actions">
    {video&&<button className="primary-button" disabled={editing} title="Edit this video" onClick={onEdit}>{editing?"Opening…":"Edit"}</button>}
    <button className="secondary-button" title={fullscreen?"Return to details view":"Open fullscreen viewer"} aria-pressed={fullscreen} onClick={onFullscreen}>{fullscreen?"Restore":"Fullscreen"}</button>
    {filmstripVisible!==undefined&&<button className="secondary-button" aria-pressed={filmstripVisible} onClick={onToggleFilmstrip}>{filmstripVisible?"Hide filmstrip":"Show filmstrip"}</button>}
    <div className="details-nav" role="group" aria-label="Browse media"><button className="icon-button" disabled={!canPrevious} onClick={onPrevious} aria-label="Previous item" title="Previous item"><ChevronLeft size={18}/></button><button className="icon-button" disabled={!canNext} onClick={onNext} aria-label="Next item" title="Next item"><ChevronRight size={18}/></button></div>
    <button className="icon-button" onClick={onClose} aria-label="Close details" title="Close details"><X size={20}/></button>
  </div></div>;
}
