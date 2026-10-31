import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {AppSidebar} from "./AppSidebar";
import {PageHeading} from "./PageHeading";
import {MediaDetailsHeader} from "./MediaDetailsHeader";
import {LocalAiSettings} from "./LocalAiSettings";
import {LibraryFilters} from "./LibraryFilters";
import {BrowseView,useBrowseView} from "./BrowseView";
import {LibraryFolders} from "./LibraryFolders";
import {workspaceFolders,initialWorkspace,type Workspace} from "./libraryWorkspace";
import {useUnifiedSearch} from "./useUnifiedSearch";
import {SearchStatus} from "./SearchStatus";
import {useManagedTags} from "./useManagedTags";
import {readMetadataBatches} from "./metadataBatches";
import {saveSetting,retrySettings} from "./userData";
import FileLocation from "./FileLocation";
import PhotoMap, {type PhotoLocation} from "./PhotoMap";
import {VideoSequences} from "./VideoSequences";
import {workspaceProject,type LoadedSequence} from "./videoSequence";
import {VideoExportPanel} from "./VideoExportPanel";
import {ProjectSaveGuard} from "./ProjectSaveGuard";
import {Modal,TaskForm,ErrorNotice,NoticeTarget,NoticeSurface,NoticeHostSetter} from "./Ui";
import { IPhonePicture, IPhoneMotion } from "./IPhoneMedia";
import {UpdateSettingsCard,UpdateToast,useAppUpdater} from "./appUpdater";
import { Images, FolderOpen, Users, Tags, Settings2, Search, Monitor, ShieldCheck, ImageOff, HardDrive, ChevronRight, X, LayoutGrid, List, ZoomIn, ZoomOut, ArrowUpDown, MapPin, CalendarDays, Heart, Clapperboard, Library } from "lucide-react";

type View = "editor" | "library" | "folders" | "collections" | "timeline" | "memories" | "map" | "people" | "tags" | "settings";
type Collection = {id:string;workspaceId:string;name:string;paths:string[]};
type SmartRule = {id:string;workspaceId:string;name:string;minRating:number;type:"all"|"photo"|"video";minMB:number};

type Theme = "light" | "dark" | "system";
type Accent = "purple" | "blue" | "teal" | "green" | "rose";
const ACCENTS:{id:Accent;label:string;color:string}[]=[
  {id:"purple",label:"Purple",color:"#6727ad"},
  {id:"blue",label:"Blue",color:"#2563eb"},
  {id:"teal",label:"Teal",color:"#0f766e"},
  {id:"green",label:"Green",color:"#15803d"},
  {id:"rose",label:"Rose",color:"#be185d"}
];
type PhotoView = "grid" | "list";
type SortBy = "relevance" | "name" | "newest" | "oldest" | "largest" | "smallest";
type Photo = { path: string; name: string; extension: string; bytes: number; modified_unix: number; capture_local?: string | null; live_asset_id?: string | null; libraryRoot?: string };
type ScanResult = { photos: Photo[]; scanned_files: number; truncated: boolean; incomplete?:boolean; errors?:string[] };
type AiBatchResult = {requested:number;indexed:number;skipped:number;failed:number;errors:string[]};
type FaceIndexBatch = {requested:number;indexed_images:number;skipped_images:number;failed_images:number;faces:number;errors:string[]};
type PersonFace = {root:string;path:string;face_index:number;x:number;y:number;width:number;height:number;score:number};
type PersonGroup = {id:string;name?:string;legacy_id?:string;members:PersonFace[]};
type FaceIndexStatus = {indexed_images:number;faces:number};
type AiModelInfo = {kind:string;installed:boolean;bytes:number;sha256:string|null;expected_sha256:string;compatible:boolean};
type AiBundleReadiness = {ready:boolean;missing:string[];incompatible:string[]};
type AiValidationReport = {semantic:AiBundleReadiness;people:AiBundleReadiness;ocr:AiBundleReadiness;directml_available:boolean;cpu_fallback:boolean;core_implementation_complete:boolean;real_world_validation_required:boolean};
const NAV: {id: View; title: string; icon: typeof Images}[] = [
  { id: "library", title: "Library", icon: Images },
  { id: "folders", title: "Folders", icon: FolderOpen },
  { id: "editor", title: "Edits", icon: Clapperboard },
  { id: "map", title: "Map", icon: MapPin },
  { id: "collections", title: "Collections", icon: Library },
  { id: "timeline", title: "By date", icon: CalendarDays },
  { id: "memories", title: "Memories", icon: Heart },
  { id: "people", title: "People", icon: Users },
  { id: "tags", title: "Tags", icon: Tags },
  { id: "settings", title: "Settings", icon: Settings2 }
];
const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const TILE_CAPTION_HEIGHT=58,TILE_ROW_GAP=18;
const EDITABLE_IMAGE_EXTENSIONS = new Set(["jpg","jpeg","png","webp","tif","tiff","bmp","avif"]);

function parseEditResizeDimension(value:string):number|undefined|null {
  const trimmed=value.trim();
  if(!trimmed)return undefined;
  if(!/^\d+$/.test(trimmed))return null;
  const parsed=Number(trimmed);
  return Number.isSafeInteger(parsed)&&parsed>=1&&parsed<=20_000?parsed:null;
}

function photoDate(photo:Photo):Date {
  if(photo.capture_local){
    const captured=new Date(photo.capture_local);
    if(!Number.isNaN(captured.getTime()))return captured;
  }
  return new Date(photo.modified_unix*1000);
}

function photoTime(photo:Photo):number {
  return photoDate(photo).getTime();
}

function photoMonthKey(photo:Photo):string {
  const date=photoDate(photo);
  return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,"0")}`;
}

function photoDateSource(photo:Photo):"EXIF metadata"|"QuickTime metadata"|"File modified fallback" {
  if(!photo.capture_local)return "File modified fallback";
  return ["mov","mp4","m4v"].includes(photo.extension.toLowerCase())
    ? "QuickTime metadata"
    : "EXIF metadata";
}

export default function App() {
  const [view, setView] = useState<View>("library");
  const [editorInitial,setEditorInitial]=useState<LoadedSequence|null>(null);
  const [editorBusy,setEditorBusy]=useState(false),[editorError,setEditorError]=useState("");
  const editorWorkspace=useRef("");
  const mainPane=useRef<HTMLDivElement>(null);
  useLayoutEffect(()=>{if(mainPane.current)mainPane.current.scrollTop=0;},[view]);
  const [noticeHost,setNoticeHost]=useState<HTMLDivElement|null>(null);
  const [form,setForm]=useState<"workspace"|"collection"|"smart"|null>(null);
  const appUpdater=useAppUpdater(isDesktop);
  const [aiOptIn,setAiOptIn]=useState(()=>localStorage.getItem("openphoto.ai.optin")==="true");
  const [aiModels,setAiModels]=useState<AiModelInfo[]>([]);
  const aiModelByKind=useMemo(()=>new Map(aiModels.map(model=>[model.kind,model])),[aiModels]);
  const semanticReady=["semantic","semantic_text","semantic_tokenizer"].every(kind=>aiModelByKind.get(kind)?.compatible);
  const peopleReady=["faces","face_embedder"].every(kind=>aiModelByKind.get(kind)?.compatible);
  const ocrReady=["ocr","ocr_recognizer"].every(kind=>aiModelByKind.get(kind)?.compatible);
  const [aiModelMessage,setAiModelMessage]=useState("");
  const [aiModelBusy,setAiModelBusy]=useState(false);

  const [aiValidationBusy,setAiValidationBusy]=useState(false);
  const [aiIndexMessage,setAiIndexMessage]=useState("");
  const [aiIndexBusy,setAiIndexBusy]=useState(false);
  const [aiIndexProgress,setAiIndexProgress]=useState({done:0,total:0});
  const aiCancelRequested=useRef(false);
  const aiEpoch=useRef(0);
  const [aiSimilar,setAiSimilar]=useState<{path:string;score:number}[]>([]);
  const [peopleGroups,setPeopleGroups]=useState<PersonGroup[]>([]);
  const [ocrBusy,setOcrBusy]=useState(false);
  const [ocrMessage,setOcrMessage]=useState("");
  const [peopleStatus,setPeopleStatus]=useState<FaceIndexStatus|null>(null);
  const [peopleBusy,setPeopleBusy]=useState(false);
  const [peopleMessage,setPeopleMessage]=useState("");
  const [peopleNames,setPeopleNames]=useState<Record<string,string>>(()=>{try{return JSON.parse(localStorage.getItem("openphoto.peopleNames")||"{}") as Record<string,string>}catch{return {}}});
  useEffect(()=>saveSetting("openphoto.peopleNames",JSON.stringify(peopleNames)),[peopleNames]);
  const [aiRuntime,setAiRuntime]=useState<{directml_compiled:boolean;directml_available:boolean;automatic_policy:string;cpu_fallback:boolean}|null>(null);
  const refreshModels=async()=>{setAiModelBusy(true);try{setAiModels(await invoke<AiModelInfo[]>("ai_model_status"));}catch(e){setAiModelMessage(String(e));}finally{setAiModelBusy(false);}};
  const refreshAiRuntime=()=>invoke<{directml_compiled:boolean;directml_available:boolean;automatic_policy:string;cpu_fallback:boolean}>("ai_runtime_status").then(setAiRuntime).catch(e=>setAiModelMessage(String(e)));
  const runAiSelfCheck=async()=>{
    setAiValidationBusy(true);setAiModelMessage("");
    try{
      const report=await invoke<AiValidationReport>("ai_validation_report");
      setAiModelMessage(report.semantic.ready&&report.people.ready&&report.ocr.ready?"Installed AI bundles passed readiness checks.":"Some AI assets are not ready. Repair or reinstall Open-Photo.");
      await Promise.all([refreshModels(),refreshAiRuntime()]);
    }catch(e){setAiModelMessage(String(e))}
    finally{setAiValidationBusy(false)}
  };
  useEffect(()=>{if(isDesktop){void refreshModels();void refreshAiRuntime()}},[]);
  function setAiEnabled(enabled:boolean){
    if(!enabled){aiCancelRequested.current=true;aiEpoch.current+=1;setAiSimilar([]);setPeopleGroups([]);}
    setAiOptIn(enabled);
  }
  const [aiDevice,setAiDevice]=useState<"auto"|"cpu">(()=>localStorage.getItem("openphoto.ai.device")==="cpu"?"cpu":"auto");
  useEffect(()=>saveSetting("openphoto.ai.optin",String(aiOptIn)),[aiOptIn]);
  useEffect(()=>saveSetting("openphoto.ai.device",aiDevice),[aiDevice]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>(()=>{try{return JSON.parse(localStorage.getItem("openphoto.workspaces")||"[]") as Workspace[]}catch{return []}});
  const [activeWorkspace,setActiveWorkspace] = useState<string>(()=>localStorage.getItem("openphoto.activeWorkspace")||"");
  const [folderFilter,setFolderFilter] = useState("");
  const [folderView,setFolderView]=useBrowseView(activeWorkspace,"folders"),[collectionView,setCollectionView]=useBrowseView(activeWorkspace,"collections");
  const [collectionFilter,setCollectionFilter]=useState("");
  const [collections,setCollections]=useState<Collection[]>(()=>{try{return JSON.parse(localStorage.getItem("openphoto.collections")||"[]") as Collection[]}catch{return []}});
  useEffect(()=>saveSetting("openphoto.collections",JSON.stringify(collections)),[collections]);
  const [geoPoints,setGeoPoints]=useState<PhotoLocation[]>([]);
  const [geoBusy,setGeoBusy]=useState(false);
  const syncRunning=useRef<number|null>(null);
  const syncGeneration=useRef(0);
  useEffect(()=>{saveSetting("openphoto.autosync","false");if(isDesktop)void invoke("set_watched_folders",{roots:[]}).catch(e=>setError(String(e)));},[]);
  const [lastSync,setLastSync]=useState("");
  const [healthReport,setHealthReport]=useState<{indexed:number;available:number;missing:number;inaccessible:number;modified:number}|null>(null);
  const [healthError,setHealthError]=useState("");
  const [healthBusy,setHealthBusy]=useState(false);
  const [syncChanges,setSyncChanges]=useState<{added:number;updated:number;unchanged:number;removed:number;incomplete:boolean}|null>(null);
  const [duplicates, setDuplicates] = useState<{paths:string[];bytes:number}[]>([]);
  const [duplicateBusy, setDuplicateBusy] = useState(false);
  const [duplicateReview,setDuplicateReview]=useState<Record<string,string>>(()=>{
    try {
      const stored=JSON.parse(localStorage.getItem("openphoto.duplicateReview")||"{}") as Record<string,unknown>;
      return Object.fromEntries(Object.entries(stored).filter((entry):entry is [string,string]=>typeof entry[1]==="string"));
    } catch { return {}; }
  });
  useEffect(()=>saveSetting("openphoto.duplicateReview",JSON.stringify(duplicateReview)),[duplicateReview]);
  const [similarGroups,setSimilarGroups]=useState<{paths:string[];max_distance:number}[]>([]);
  const [mediaType,setMediaType] = useState<"all"|"photo"|"video">("all");
  const [organizeFilter,setOrganizeFilter] = useState<"all"|"favorites"|"rated">("all");
  const [photoRatings,setPhotoRatings] = useState<Record<string,number>>(()=>{
    try {
      const saved=JSON.parse(localStorage.getItem("openphoto.ratings")||"{}") as Record<string,number>;
      return Object.fromEntries(Object.entries(saved).filter(([,rating])=>Number.isInteger(rating)&&rating>=0&&rating<=5));
    } catch { return {}; }
  });
  useEffect(()=>saveSetting("openphoto.ratings",JSON.stringify(photoRatings)),[photoRatings]);
  const ratePhoto=(path:string,rating:number)=>setPhotoRatings(previous=>({...previous,[path]:previous[path]===rating?0:rating}));
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("openphoto.theme") as Theme) || "system");
  const [accent,setAccent]=useState<Accent>(()=>{const value=localStorage.getItem("openphoto.accent") as Accent|null;return ACCENTS.some(item=>item.id===value)?value!:"purple";});
  const [root, setRoot] = useState("");
  useEffect(()=>{saveSetting("openphoto.workspaces",JSON.stringify(workspaces))},[workspaces]);
  useEffect(()=>{saveSetting("openphoto.activeWorkspace",activeWorkspace)},[activeWorkspace]);
  const [result, setResult] = useState<ScanResult | null>(null);
  const [query, setQuery] = useState("");
  const [smartFilter,setSmartFilter]=useState<""|"recent"|"thisDay"|"large"|"unrated"|"thisMonth">("");
  const [timelineMonth,setTimelineMonth]=useState("");
  const [memoryYear,setMemoryYear]=useState<number|null>(null);
  const [selectedTag,setSelectedTag]=useState("");
  const [tagRefreshKey,setTagRefreshKey]=useState(0);
  const {managedTags,tagIndexBusy,tagIndexError}=useManagedTags(result?.photos,root,activeWorkspace,isDesktop&&(view==="tags"||!!selectedTag||!!query.trim()),tagRefreshKey);
  const [selectedSmartRule,setSelectedSmartRule]=useState("");
  const [smartRules,setSmartRules]=useState<SmartRule[]>(()=>{
    try {
      const saved=JSON.parse(localStorage.getItem("openphoto.smartRules")||"[]") as SmartRule[];
      return Array.isArray(saved)?saved.filter(item=>typeof item.id==="string"&&typeof item.name==="string"&&["all","photo","video"].includes(item.type)&&Number.isInteger(item.minRating)&&item.minRating>=0&&item.minRating<=5&&Number.isFinite(item.minMB)&&item.minMB>=0):[];
    }catch{return [];}
  });
  useEffect(()=>saveSetting("openphoto.smartRules",JSON.stringify(smartRules)),[smartRules]);
  const [photoView, setPhotoView] = useState<PhotoView>("grid");
  const [sortBy, setSortBy] = useState<SortBy>("newest");
  const browsingSort=useRef<SortBy>("newest");
  const [tileSize, setTileSize] = useState(180);
  const [selection, setSelection] = useState<string[]>([]);
  const [tagText, setTagText] = useState("");
  const [applying, setApplying] = useState(false);
  const [tagMessage, setTagMessage] = useState("");
  const [scrollTop, setScrollTop] = useState(0);
  const [gridWidth, setGridWidth] = useState(940);
  const [gridHost,setGridHost]=useState<HTMLDivElement|null>(null);
  useLayoutEffect(()=>{
    if(!gridHost)return;
    const measure=()=>setGridWidth(gridHost.clientWidth);
    measure();
    const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(measure):null;
    observer?.observe(gridHost);window.addEventListener("resize",measure);
    return()=>{observer?.disconnect();window.removeEventListener("resize",measure);};
  },[gridHost]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [storageError,setStorageError]=useState("");
  const [backupMessage,setBackupMessage]=useState("");
  const exportMetadataBackup=async()=>{
    try{
      await retrySettings();
      const destination=await save({title:"Export library metadata backup",defaultPath:"open-photo-metadata.json",filters:[{name:"Metadata backup",extensions:["json"]}]});
      if(!destination)return;
      await invoke("export_user_data",{destination});
      setBackupMessage("Library metadata backup saved. Original media files are not included.");
    }catch(error){setBackupMessage(String(error))}
  };

  useEffect(()=>{
    const onError=(event:Event)=>setStorageError((event as CustomEvent<string>).detail);
    window.addEventListener("openphoto-storage-error",onError);
    return()=>window.removeEventListener("openphoto-storage-error",onError);
  },[]);

  const [selected, setSelected] = useState<Photo | null>(null);
  const [fullScreenPreview,setFullScreenPreview]=useState(false);
  useEffect(()=>{if(!selected){setFullScreenPreview(false);}},[selected?.path]);
  const [editRotation,setEditRotation]=useState<0|90|180|270>(0);
  const [editFlipH,setEditFlipH]=useState(false);
  const [editFlipV,setEditFlipV]=useState(false);
  const [editBrightness,setEditBrightness]=useState(0);
  const [editContrast,setEditContrast]=useState(0);
  const [editCropLeft,setEditCropLeft]=useState(0);
  const [editCropTop,setEditCropTop]=useState(0);
  const [editCropRight,setEditCropRight]=useState(0);
  const [editCropBottom,setEditCropBottom]=useState(0);
  const [editResizeWidth,setEditResizeWidth]=useState("");
  const [editResizeHeight,setEditResizeHeight]=useState("");
  const [editGrayscale,setEditGrayscale]=useState(false);
  const [editHue,setEditHue]=useState(0);
  const [editBlur,setEditBlur]=useState(0);
  const [editSharpen,setEditSharpen]=useState(0);
  const [editSharpenThreshold,setEditSharpenThreshold]=useState(0);
  const [editJpegQuality,setEditJpegQuality]=useState(92);
  const [editPreserveModifiedTime,setEditPreserveModifiedTime]=useState(true);
  const [editShowOriginal,setEditShowOriginal]=useState(false);
  const [editPreviewSrc,setEditPreviewSrc]=useState("");
  const [editOutputDimensions,setEditOutputDimensions]=useState<{width:number;height:number}|null>(null);
  const [editSourceDimensions,setEditSourceDimensions]=useState<{width:number;height:number}|null>(null);
  const [editPreviewBusy,setEditPreviewBusy]=useState(false);
  const [editPreviewError,setEditPreviewError]=useState("");
  const [editExportBusy,setEditExportBusy]=useState(false);
  const [editExportMessage,setEditExportMessage]=useState("");
  useEffect(()=>{setEditRotation(0);setEditFlipH(false);setEditFlipV(false);setEditBrightness(0);setEditContrast(0);setEditCropLeft(0);setEditCropTop(0);setEditCropRight(0);setEditCropBottom(0);setEditResizeWidth("");setEditResizeHeight("");setEditGrayscale(false);setEditHue(0);setEditBlur(0);setEditSharpen(0);setEditSharpenThreshold(0);setEditJpegQuality(92);setEditPreserveModifiedTime(true);setEditShowOriginal(false);setEditPreviewSrc("");setEditOutputDimensions(null);setEditSourceDimensions(null);setEditPreviewError("");setEditExportMessage("")},[selected?.path]);
  useEffect(()=>{
    if(!selected||!isDesktop||!EDITABLE_IMAGE_EXTENSIONS.has(selected.extension.toLowerCase())){setEditPreviewSrc("");setEditOutputDimensions(null);setEditSourceDimensions(null);setEditPreviewBusy(false);return;}
    let active=true;
    const timer=window.setTimeout(()=>{
      setEditPreviewBusy(true);setEditPreviewError("");
      const width=parseEditResizeDimension(editResizeWidth);
      const height=parseEditResizeDimension(editResizeHeight);
      if(width===null||height===null){
        setEditPreviewError("Resize width and height must be whole numbers from 1 to 20,000 pixels.");
        setEditPreviewSrc("");setEditOutputDimensions(null);setEditPreviewBusy(false);return;
      }
      void invoke<{data_uri:string;width:number;height:number;source_width:number;source_height:number}>("preview_edited_photo",{root:selected.libraryRoot||root,path:selected.path,maxEdge:1400,options:{rotation:editRotation,flipHorizontal:editFlipH,flipVertical:editFlipV,brightness:editBrightness,contrast:editContrast,cropLeft:editCropLeft,cropTop:editCropTop,cropRight:editCropRight,cropBottom:editCropBottom,resizeWidth:width,resizeHeight:height,grayscale:editGrayscale,hueRotate:editHue,blurSigma:editBlur/10,sharpenSigma:editSharpen/10,sharpenThreshold:editSharpenThreshold}})
        .then(preview=>{if(active){setEditPreviewSrc(preview.data_uri);setEditOutputDimensions({width:preview.width,height:preview.height});setEditSourceDimensions({width:preview.source_width,height:preview.source_height})}})
        .catch(e=>{if(active){setEditPreviewError(String(e));setEditPreviewSrc("");setEditOutputDimensions(null);setEditSourceDimensions(null)}})
        .finally(()=>{if(active)setEditPreviewBusy(false)});
    },220);
    return()=>{active=false;window.clearTimeout(timer)};
  },[selected?.path,selected?.libraryRoot,selected?.extension,root,editRotation,editFlipH,editFlipV,editBrightness,editContrast,editCropLeft,editCropTop,editCropRight,editCropBottom,editResizeWidth,editResizeHeight,editGrayscale,editHue,editBlur,editSharpen,editSharpenThreshold]);
  useEffect(()=>{
    saveSetting("openphoto.accent",accent);
    document.documentElement.dataset.accent=accent;
  },[accent]);
  useEffect(() => {
    saveSetting("openphoto.theme", theme);
    const body = document.documentElement;
    const update = () => body.dataset.theme = theme === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : theme;
    update();
    const media = matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [theme]);

  const applyCropPreset=(ratio?:number)=>{
    if(!editSourceDimensions){
      setEditPreviewError("Source dimensions are still loading. Try the crop preset again in a moment.");
      return;
    }
    if(!ratio){
      setEditCropLeft(0);setEditCropTop(0);setEditCropRight(0);setEditCropBottom(0);
      return;
    }
    const rotated=editRotation===90||editRotation===270;
    const width=rotated?editSourceDimensions.height:editSourceDimensions.width;
    const height=rotated?editSourceDimensions.width:editSourceDimensions.height;
    const currentRatio=width/height;
    if(currentRatio>ratio){
      const desiredWidth=height*ratio;
      const edge=Math.max(0,Math.min(45,((width-desiredWidth)/(2*width))*100));
      setEditCropLeft(edge);setEditCropRight(edge);setEditCropTop(0);setEditCropBottom(0);
    }else{
      const desiredHeight=width/ratio;
      const edge=Math.max(0,Math.min(45,((height-desiredHeight)/(2*height))*100));
      setEditCropTop(edge);setEditCropBottom(edge);setEditCropLeft(0);setEditCropRight(0);
    }
  };

  const exportEditedPhoto=async()=>{
    if(!selected)return;
    const resizeWidth=parseEditResizeDimension(editResizeWidth);
    const resizeHeight=parseEditResizeDimension(editResizeHeight);
    if(resizeWidth===null||resizeHeight===null){
      setEditExportMessage("Resize width and height must be whole numbers from 1 to 20,000 pixels.");
      return;
    }
    const stem=selected.name.replace(/\.[^.]+$/,"");
    const destination=await save({defaultPath:`${stem}-edited.jpg`,filters:[{name:"Edited photo",extensions:["jpg","jpeg","png","webp","bmp","tif","tiff"]}]});
    if(typeof destination!=="string")return;
    setEditExportBusy(true);setEditExportMessage("");
    try{
      const result=await invoke<{path:string;width:number;height:number;bytes:number;preserved_modified_time:boolean}>("export_edited_copy",{root:selected.libraryRoot||root,path:selected.path,destination,options:{rotation:editRotation,flipHorizontal:editFlipH,flipVertical:editFlipV,brightness:editBrightness,contrast:editContrast,cropLeft:editCropLeft,cropTop:editCropTop,cropRight:editCropRight,cropBottom:editCropBottom,resizeWidth,resizeHeight,grayscale:editGrayscale,hueRotate:editHue,blurSigma:editBlur/10,sharpenSigma:editSharpen/10,sharpenThreshold:editSharpenThreshold,jpegQuality:editJpegQuality,preserveModifiedTime:editPreserveModifiedTime}});
      setEditExportMessage(`Saved new edited copy: ${result.width}×${result.height} · ${Math.round(result.bytes/1024)} KB${result.preserved_modified_time?" · source date preserved":""} · ${result.path}`);
    }catch(e){setEditExportMessage(String(e))}
    finally{setEditExportBusy(false)}
  };
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
  editorWorkspace.current=activeWorkspace;
  const workspace = workspaces.find(w=>w.id===activeWorkspace);
  const roots = workspaceFolders(workspace);
  const [searchRevision,setSearchRevision]=useState(0);
  const discovery=useUnifiedSearch({photos:result?.photos,root,workspace:activeWorkspace,query,tags:managedTags,ratings:photoRatings,aiEnabled:aiOptIn,desktop:isDesktop,device:aiDevice,indexing:aiIndexBusy||ocrBusy||peopleBusy,refresh:searchRevision});
  function updateSearch(value:string){if(!query.trim()&&value.trim()){browsingSort.current=sortBy==="relevance"?"newest":sortBy;setSortBy("relevance");}else if(query.trim()&&!value.trim()&&sortBy==="relevance")setSortBy(browsingSort.current);setQuery(value);}

  const indexAiPhotos=async(photos:Photo[],label:string)=>{
    const epoch=aiEpoch.current;const libraryGeneration=syncGeneration.current;
    const supported=new Set(["jpg","jpeg","png","webp","gif","tif","tiff","bmp","avif"]);
    const grouped=new Map<string,string[]>();
    for(const photo of photos){
      if(!supported.has(photo.extension.toLowerCase()))continue;
      const parent=photo.libraryRoot||root;
      if(!parent)continue;
      const paths=grouped.get(parent)||[];
      paths.push(photo.path);
      grouped.set(parent,paths);
    }
    const requested=[...grouped.values()].reduce((total,paths)=>total+paths.length,0);
    if(!requested){
      setAiIndexMessage("No currently supported AI-decodable photos were found in this selection.");
      return;
    }
    aiCancelRequested.current=false;
    setAiIndexBusy(true);
    setAiIndexProgress({done:0,total:requested});
    setAiIndexMessage(`Checking ${requested} ${label} photos for local AI indexing…`);
    try{
      const total:AiBatchResult={requested:0,indexed:0,skipped:0,failed:0,errors:[]};
      let completed=0;
      outer: for(const [parent,paths] of grouped){
        for(let offset=0;offset<paths.length;offset+=64){
          if(aiCancelRequested.current||epoch!==aiEpoch.current||libraryGeneration!==syncGeneration.current)break outer;
          const slice=paths.slice(offset,offset+64);
          const batch=await invoke<AiBatchResult>("ai_index_semantic_batch",{root:parent,paths:slice,device:aiDevice});
          total.requested+=batch.requested;
          total.indexed+=batch.indexed;
          total.skipped+=batch.skipped;
          total.failed+=batch.failed;
          total.errors.push(...batch.errors);
          completed+=slice.length;
          setAiIndexProgress({done:completed,total:requested});
          setAiIndexMessage(`AI indexing ${Math.min(completed,requested)} of ${requested}: ${total.indexed} indexed, ${total.skipped} already current${total.failed?`, ${total.failed} failed`:""}.`);
        }
      }
      if(aiCancelRequested.current||epoch!==aiEpoch.current||libraryGeneration!==syncGeneration.current){
        setAiIndexMessage(`AI indexing cancelled after ${completed} of ${requested} photos. Completed embeddings remain safely stored.`);
      }else{
        setAiIndexMessage(`AI indexing checked ${total.requested} photos: ${total.indexed} indexed, ${total.skipped} already current${total.failed?`, ${total.failed} failed`:""}.`);
      }
    }catch(e){setAiIndexMessage(String(e))}
    finally{setAiIndexBusy(false);setSearchRevision(value=>value+1)}
  };
  const indexOcrWorkspace=async()=>{
    const epoch=aiEpoch.current;const libraryGeneration=syncGeneration.current;
    if(!result||!roots.length)return;
    const supported=new Set(["jpg","jpeg","png","webp","gif","tif","tiff","bmp","avif"]);
    const grouped=new Map<string,string[]>();
    for(const photo of result.photos){
      if(!supported.has(photo.extension.toLowerCase()))continue;
      const parent=photo.libraryRoot||root;
      if(!parent)continue;
      const paths=grouped.get(parent)||[];
      paths.push(photo.path);
      grouped.set(parent,paths);
    }
    const requested=[...grouped.values()].reduce((total,paths)=>total+paths.length,0);
    if(!requested){setOcrMessage("No currently supported photos are available for OCR indexing.");return;}
    setOcrBusy(true);setOcrMessage(`Starting local OCR for ${requested} photos…`);
    try{
      let indexed=0,skipped=0,failed=0,regions=0,completed=0;
      for(const [parent,paths] of grouped){
        for(let offset=0;offset<paths.length;offset+=24){
          if(epoch!==aiEpoch.current||libraryGeneration!==syncGeneration.current){setOcrMessage("OCR indexing stopped.");return;}
          const slice=paths.slice(offset,offset+24);
          const batch=await invoke<{requested:number;indexed:number;skipped:number;failed:number;regions:number;errors:string[]}>("ai_index_ocr_batch",{root:parent,paths:slice,device:aiDevice});
          indexed+=batch.indexed;skipped+=batch.skipped;failed+=batch.failed;regions+=batch.regions;completed+=slice.length;
          setOcrMessage(`OCR ${completed}/${requested}: ${regions} text regions found, ${skipped} photos already current${failed?`, ${failed} failed`:""}.`);
        }
      }
      setOcrMessage(`OCR indexing complete: ${indexed} photos processed, ${skipped} already current, ${regions} text regions found${failed?`, ${failed} failed`:""}.`);
    }catch(e){setOcrMessage(String(e))}finally{setOcrBusy(false);setSearchRevision(value=>value+1)}
  };
  const refreshPeople=async()=>{
    const generation=syncGeneration.current;
    if(!roots.length){setPeopleGroups([]);setPeopleStatus(null);return;}
    setPeopleMessage("");
    try{
      const [groups,status]=await Promise.all([
        invoke<PersonGroup[]>("ai_people_groups",{roots,legacyNames:peopleNames}),
        invoke<FaceIndexStatus>("ai_face_index_status",{roots})
      ]);
      if(generation!==syncGeneration.current)return;
      setPeopleGroups(groups);setSearchRevision(value=>value+1);
      setPeopleStatus(status);
    }catch(e){setPeopleMessage(String(e))}
  };
  const indexPeopleWorkspace=async()=>{
    const epoch=aiEpoch.current;const libraryGeneration=syncGeneration.current;
    if(!result||!roots.length)return;
    const supported=new Set(["jpg","jpeg","png","webp","gif","tif","tiff","bmp","avif"]);
    const grouped=new Map<string,string[]>();
    for(const photo of result.photos){
      if(!supported.has(photo.extension.toLowerCase()))continue;
      const parent=photo.libraryRoot||root;
      if(!parent)continue;
      const paths=grouped.get(parent)||[];
      paths.push(photo.path);
      grouped.set(parent,paths);
    }
    const requested=[...grouped.values()].reduce((total,paths)=>total+paths.length,0);
    if(!requested){setPeopleMessage("No currently supported photos are available for People indexing.");return;}
    setPeopleBusy(true);
    setPeopleMessage(`Starting local face indexing for ${requested} photos…`);
    try{
      const total:FaceIndexBatch={requested:0,indexed_images:0,skipped_images:0,failed_images:0,faces:0,errors:[]};
      let completed=0;
      for(const [parent,paths] of grouped){
        for(let offset=0;offset<paths.length;offset+=32){
          if(epoch!==aiEpoch.current||libraryGeneration!==syncGeneration.current){setPeopleMessage("People indexing stopped.");return;}
          const slice=paths.slice(offset,offset+32);
          const batch=await invoke<FaceIndexBatch>("ai_index_faces_batch",{root:parent,paths:slice,device:aiDevice});
          total.requested+=batch.requested;
          total.indexed_images+=batch.indexed_images;
          total.skipped_images+=batch.skipped_images;
          total.failed_images+=batch.failed_images;
          total.faces+=batch.faces;
          total.errors.push(...batch.errors);
          completed+=slice.length;
          setPeopleMessage(`People indexing ${completed}/${requested}: ${total.faces} faces found, ${total.skipped_images} photos already current${total.failed_images?`, ${total.failed_images} failed`:""}.`);
        }
      }
      await refreshPeople();
      setPeopleMessage(`People indexing complete: ${total.indexed_images} photos processed, ${total.skipped_images} already current, ${total.faces} faces added${total.failed_images?`, ${total.failed_images} failed`:""}.`);
    }catch(e){setPeopleMessage(String(e))}
    finally{setPeopleBusy(false)}
  };
  useEffect(()=>{if(view==="people"&&aiOptIn&&roots.length)void refreshPeople()},[view,activeWorkspace,aiOptIn]);
  const syncFolders = async (paths: string[],restore=false) => {
    if (!isDesktop || !paths.length || syncRunning.current===syncGeneration.current) return;
    const generation=syncGeneration.current;
    syncRunning.current=generation;
    setBusy(true); setError("");
    try {
      const scans = await Promise.allSettled(paths.map(async path=>{
        const response=restore?{scan:await invoke<ScanResult>("saved_library",{root:path}),changes:{added:0,updated:0,unchanged:0,removed:0,incomplete:false}}:await invoke<{scan:ScanResult;changes:{added:number;updated:number;unchanged:number;removed:number;incomplete:boolean}}>("scan_indexed_with_changes",{root:path});
        const data=response.scan;
        return {...data,libraryRoot:path,changes:response.changes,photos:data.photos.map(photo=>({...photo,libraryRoot:path}))};
      }));
      const success=scans.filter(s=>s.status==="fulfilled");
      const failures=scans.filter((s):s is PromiseRejectedResult=>s.status==="rejected");
      if (!success.length) throw Error(failures.map(x=>String(x.reason)).join("; "));
      const merged=new Map<string,Photo>();
      const retainedRoots=new Set(paths.filter((_,index)=>scans[index].status==="rejected"));
      for(const scan of success)if(scan.value.incomplete)retainedRoots.add(scan.value.libraryRoot);
      for(const photo of result?.photos||[])if(retainedRoots.has(photo.libraryRoot||root))merged.set(photo.path,photo);
      for(const scan of success)for(const photo of scan.value.photos)merged.set(photo.path,photo);
      if(generation!==syncGeneration.current)return;
      setResult({photos:[...merged.values()],scanned_files:success.reduce((n,s)=>n+s.value.scanned_files,0),truncated:success.some(s=>s.value.truncated),incomplete:failures.length>0||success.some(s=>s.value.incomplete),errors:success.flatMap(s=>s.value.errors||[])});
      const scanErrors=[...failures.map(x=>String(x.reason)),...success.flatMap(s=>s.value.errors||[])];
      if(scanErrors.length)setError(scanErrors.slice(0,10).join("; "));
      if(!restore)setSyncChanges(success.reduce<{added:number;updated:number;unchanged:number;removed:number;incomplete:boolean}>((summary,entry)=>({added:summary.added+entry.value.changes.added,updated:summary.updated+entry.value.changes.updated,unchanged:summary.unchanged+entry.value.changes.unchanged,removed:summary.removed+entry.value.changes.removed,incomplete:summary.incomplete||entry.value.changes.incomplete}),{added:0,updated:0,unchanged:0,removed:0,incomplete:false}));else setSyncChanges(null);
      if(!restore)setLastSync(new Date().toLocaleString());setDuplicates([]);setSimilarGroups([]);
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
      setActiveWorkspace(id);setRoot(nextRoots[0]);setSelection([]);setSelected(null);setFolderFilter("");setSelectedTag("");
      await syncFolders(nextRoots);
    }catch(e){setError(String(e))}
  };
  const activateWorkspace=async(item:Workspace)=>{
    syncGeneration.current+=1;
    aiCancelRequested.current=true;aiEpoch.current+=1;
    setEditorInitial(null);setEditorError("");setView("library");setActiveWorkspace(item.id);setRoot(item.root);setSelection([]);setSelected(null);setResult(null);setFolderFilter("");setSelectedTag("");setPeopleGroups([]);setCollectionFilter("");setSelectedSmartRule("");setPeopleStatus(null);
    setAiSimilar([]);setDuplicates([]);setSimilarGroups([]);setLastSync("");setSyncChanges(null);setError("");
    const folders=workspaceFolders(item);
    if(folders.length)await syncFolders(folders,true);else setBusy(false);
  };
  const openWorkspace=async(id:string)=>{
    const item=workspaces.find(w=>w.id===id);if(item)await activateWorkspace(item);
  };
  function createWorkspace(name?:string){
    if(!name){setForm("workspace");return;}
    const item:Workspace={id:crypto.randomUUID(),name:name.trim(),root:"",roots:[]};
    setWorkspaces(old=>[...old,item]);setView("library");void activateWorkspace(item);
  };
  const restoredLibrary=useRef(false);
  useEffect(()=>{
    if(restoredLibrary.current)return;
    restoredLibrary.current=true;
    const saved=initialWorkspace(workspaces,activeWorkspace);
    if(saved)void openWorkspace(saved.id);
  },[]);
  function removeLibraryFolder(path:string){
    const remaining=roots.filter(folder=>folder!==path);
    syncGeneration.current+=1;
    aiCancelRequested.current=true;aiEpoch.current+=1;
    setLastSync("");setSyncChanges(null);
    setWorkspaces(old=>old.map(item=>item.id===activeWorkspace?{...item,root:remaining[0]||"",roots:remaining}:item));
    setRoot(remaining[0]||"");setResult(null);setSelection([]);setSelected(null);setFolderFilter("");setSelectedTag("");setCollectionFilter("");setSelectedSmartRule("");setPeopleGroups([]);setPeopleStatus(null);setAiSimilar([]);setDuplicates([]);setSimilarGroups([]);setSimilarGroups([]);
    if(remaining.length)void syncFolders(remaining,true);else setBusy(false);
  }
  useEffect(()=>{
    if(view!=="map"||!result){setGeoPoints([]);return;}
    let active=true;
    setGeoBusy(true);
    void readMetadataBatches((result.photos).filter(photo=>["jpg","jpeg","tif","tiff"].includes(photo.extension)),root,1000,
      (parent,paths)=>invoke<PhotoLocation[]>("gps_for_photos",{root:parent,paths}),()=>active)
      .then(({values,errors})=>{
        if(!active)return;
        setGeoPoints(values);
        if(errors.length)setError(`Some map locations could not be loaded: ${errors.join("; ")}`);
      }).finally(()=>{if(active)setGeoBusy(false)});
    return ()=>{active=false};
  },[root,result,view]);
  const folders=useMemo(()=>[...new Set((result?.photos||[]).map(p=>p.path.slice(0,Math.max(p.path.lastIndexOf("\\"),p.path.lastIndexOf("/"))).replace(p.libraryRoot||root,"").replace(/^[\\/]/,"")||"(root)"))].sort(),[result,root]);
  // Prefer Apple's shared Live Photo content identifier when available.
  // Exporters sometimes strip it, so unmatched files retain the conservative
  // same-folder + same-stem fallback. Source files remain separate and read-only.
  const livePhotoPairing=useMemo(()=>{
    const photos=result?.photos||[];
    const matches=new Map<string,Photo>();
    const usedVideos=new Set<string>();
    const byAssetId=new Map<string,{images:Photo[];videos:Photo[]}>();

    for(const photo of photos){
      const extension=photo.extension.toLowerCase();
      if(!["mov","mp4","m4v","heic","heif","jpg","jpeg"].includes(extension))continue;
      const assetId=photo.live_asset_id?.trim();
      if(!assetId)continue;
      const group=byAssetId.get(assetId)||{images:[],videos:[]};
      if(["mov","mp4","m4v"].includes(extension))group.videos.push(photo);
      else group.images.push(photo);
      byAssetId.set(assetId,group);
    }

    let metadataPairs=0;
    for(const group of byAssetId.values()){
      if(group.images.length!==1||group.videos.length!==1)continue;
      matches.set(group.images[0].path,group.videos[0]);
      usedVideos.add(group.videos[0].path);
      metadataPairs+=1;
    }

    const byStem=new Map<string,{images:Photo[];videos:Photo[]}>();
    for(const photo of photos){
      if(matches.has(photo.path)||usedVideos.has(photo.path))continue;
      const extension=photo.extension.toLowerCase();
      if(!["mov","heic","heif","jpg","jpeg"].includes(extension))continue;
      const normalized=photo.path.toLowerCase().replace(/\\/g,"/");
      const stem=normalized.slice(0,normalized.lastIndexOf("."));
      const group=byStem.get(stem)||{images:[],videos:[]};
      if(extension==="mov")group.videos.push(photo);
      else group.images.push(photo);
      byStem.set(stem,group);
    }

    let fallbackPairs=0;
    for(const group of byStem.values()){
      if(group.images.length!==1||group.videos.length!==1)continue;
      matches.set(group.images[0].path,group.videos[0]);
      usedVideos.add(group.videos[0].path);
      fallbackPairs+=1;
    }
    return {matches,metadataPairs,fallbackPairs};
  },[result]);
  const liveVideoByImage=livePhotoPairing.matches;
  const hiddenLiveVideos=useMemo(()=>new Set([...liveVideoByImage.values()].map(video=>video.path)),[liveVideoByImage]);
  const livePhotoPairs=liveVideoByImage.size;
  const onThisDayMemories=useMemo(()=>{
    const today=new Date();
    const groups=new Map<number,Photo[]>();
    for(const photo of result?.photos||[]){
      const date=photoDate(photo);
      if(date.getFullYear()>=today.getFullYear())continue;
      if(date.getMonth()!==today.getMonth()||date.getDate()!==today.getDate())continue;
      const items=groups.get(date.getFullYear())||[];
      items.push(photo);
      groups.set(date.getFullYear(),items);
    }
    return [...groups.entries()]
      .sort((a,b)=>b[0]-a[0])
      .map(([year,photos])=>({year,photos:photos.sort((a,b)=>photoTime(b)-photoTime(a))}));
  },[result]);
  const memoryYears=useMemo(()=>{
    const counts=new Map<number,{count:number;exif:number;quicktime:number;fallback:number}>();
    for(const photo of result?.photos||[]){
      const year=photoDate(photo).getFullYear();
      const current=counts.get(year)||{count:0,exif:0,quicktime:0,fallback:0};
      current.count+=1;
      const source=photoDateSource(photo);
      if(source==="EXIF metadata")current.exif+=1;
      else if(source==="QuickTime metadata")current.quicktime+=1;
      else current.fallback+=1;
      counts.set(year,current);
    }
    return [...counts.entries()]
      .sort((a,b)=>b[0]-a[0])
      .map(([year,summary])=>({year,...summary}));
  },[result]);
  const tagBranches=useMemo(()=>{
    const pathsByBranch=new Map<string,Set<string>>();
    for(const [path,tags] of Object.entries(managedTags)){
      for(const rawTag of tags){
        const parts=rawTag.split("/").map(part=>part.trim()).filter(Boolean);
        for(let depth=1;depth<=parts.length;depth++){
          const branch=parts.slice(0,depth).join("/");
          const paths=pathsByBranch.get(branch)||new Set<string>();
          paths.add(path);
          pathsByBranch.set(branch,paths);
        }
      }
    }
    return [...pathsByBranch.entries()]
      .map(([tag,paths])=>({tag,count:paths.size,depth:tag.split("/").length}))
      .sort((a,b)=>a.tag.localeCompare(b.tag));
  },[managedTags]);
  const currentCollection=collections.find(c=>c.id===collectionFilter&&c.workspaceId===activeWorkspace);
  const visible = useMemo(() => {
    const photos = result?.photos.filter(p => {
      const mediaDate = photoDate(p);
      const mediaTime = mediaDate.getTime();
      const today = new Date();
      const smartMatch = smartFilter === "" ||
        (smartFilter === "recent" && mediaTime >= today.getTime() - 30 * 86400000) ||
        (smartFilter === "thisDay" && mediaDate.getMonth() === today.getMonth() && mediaDate.getDate() === today.getDate() && mediaDate.getFullYear() < today.getFullYear()) ||
        (smartFilter === "large" && p.bytes >= 50 * 1024 * 1024) ||
        (smartFilter === "unrated" && !(photoRatings[p.path] > 0)) ||
        (smartFilter === "thisMonth" && mediaDate.getMonth() === today.getMonth() && mediaDate.getFullYear() < today.getFullYear());
      const video = ["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension);
      const rule=smartRules.find(item=>item.id===selectedSmartRule && item.workspaceId===activeWorkspace);
      const ruleMatch=!rule || ((rule.type==="all" || (rule.type==="video" ? video : !video)) && (photoRatings[p.path]||0)>=rule.minRating && p.bytes>=rule.minMB*1024*1024);
      const timelineMatch=!timelineMonth || photoMonthKey(p)===timelineMonth;
      const memoryYearMatch=memoryYear===null || mediaDate.getFullYear()===memoryYear;
      const tagMatch=!selectedTag || (managedTags[p.path]||[]).some(tag=>tag===selectedTag||tag.startsWith(selectedTag+"/"));
      return smartMatch && ruleMatch && timelineMatch && memoryYearMatch && tagMatch &&
        (mediaType === "video" || !hiddenLiveVideos.has(p.path)) &&
        (!discovery.active||!!discovery.match(p)) &&
        (organizeFilter === "all" || (organizeFilter === "favorites" ? photoRatings[p.path] === 5 : (photoRatings[p.path] || 0) > 0)) &&
        (!folderFilter || p.path.includes(folderFilter)) &&
        (!currentCollection || currentCollection.paths.includes(p.path)) &&
        (mediaType === "all" || (mediaType === "video" ? video : !video));
    }) || [];
    return photos.sort((a,b) => {
      switch (sortBy) {
        case "relevance": return discovery.active?(discovery.match(b)?.score||0)-(discovery.match(a)?.score||0)||a.path.localeCompare(b.path):photoTime(b)-photoTime(a)||a.path.localeCompare(b.path);
        case "name": return a.name.localeCompare(b.name) || a.path.localeCompare(b.path);
        case "newest": return photoTime(b) - photoTime(a) || a.path.localeCompare(b.path);
        case "oldest": return photoTime(a) - photoTime(b) || a.path.localeCompare(b.path);
        case "largest": return b.bytes - a.bytes || a.path.localeCompare(b.path);
        case "smallest": return a.bytes - b.bytes || a.path.localeCompare(b.path);
      }
    });
  }, [result, query, discovery.results, sortBy, folderFilter, mediaType, smartFilter, smartRules, selectedSmartRule, activeWorkspace, organizeFilter, photoRatings, currentCollection, hiddenLiveVideos, timelineMonth, memoryYear, selectedTag, managedTags]);
  useLayoutEffect(() => {if(gridHost)gridHost.scrollTop=0;setScrollTop(0);}, [gridHost,query,sortBy,photoView,tileSize,root,folderFilter,mediaType,organizeFilter,smartFilter,timelineMonth,memoryYear,selectedTag]);
  const gridGap = 14;
  const columns = Math.max(1, Math.floor((gridWidth - 48 + gridGap) / (tileSize + gridGap)));
  const rowHeight = Math.max(tileSize, (gridWidth - 48) / columns - gridGap) + TILE_CAPTION_HEIGHT + TILE_ROW_GAP;
  const windowStart = Math.max(0, Math.floor(scrollTop / rowHeight) - 2);
  const rowCount = Math.ceil(visible.length / columns);
  const windowEnd = Math.min(rowCount, windowStart + Math.ceil(700 / rowHeight) + 6);
  const windowPhotos = visible.slice(windowStart * columns, windowEnd * columns);
  useEffect(()=>{
    if(!selected)return;
    const handle=(event:KeyboardEvent)=>{
      if(event.defaultPrevented)return;
      if(event.key==="Escape"){setFullScreenPreview(false);setSelected(null);return;}
      if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight")return;
      if((event.target as HTMLElement)?.closest("input,select,textarea,.video-player,.video-workspace"))return;
      const index=visible.findIndex(photo=>photo.path===selected.path);
      const next=index+(event.key==="ArrowRight"?1:-1);
      if(next>=0&&next<visible.length){event.preventDefault();setSelected(visible[next]);}
    };
    window.addEventListener("keydown",handle);
    return ()=>window.removeEventListener("keydown",handle);
  },[selected,visible]);
  const toggleSelection = (path: string) => setSelection(old => old.includes(path) ? old.filter(p => p !== path) : [...old, path]);
  const applyTags = async () => {
    if (!root || !selection.length || !tagText.trim()) return;
    const tags = [...new Set(tagText.split(",").map(x=>x.trim()).filter(Boolean))];
    if (!tags.length) return;
    if (!window.confirm(`Add ${tags.join(", ")} to ${selection.length} selected items? Open-Photo can create new sidecars or merge into sidecars it manages. Foreign/unknown XMP files will be skipped, and originals are never modified.`)) return;
    setApplying(true); setTagMessage("");
    try {
      const generation=syncGeneration.current;
      const photosByPath=new Map(result?.photos.map(photo=>[photo.path,photo]));
      const selectedPaths=selection.map(path=>({path,libraryRoot:photosByPath.get(path)?.libraryRoot||root}));
      const {values:outcomes,errors:batchErrors}=await readMetadataBatches(selectedPaths,root,1000,async(parent,paths)=>[
        await invoke<{created:number;updated:number;skipped:number;errors:string[]}>("bulk_create_tags",{root:parent,paths,tags})
      ]);
      if(generation!==syncGeneration.current)return;
      const r={created:outcomes.reduce((n,x)=>n+x.created,0),updated:outcomes.reduce((n,x)=>n+x.updated,0),skipped:outcomes.reduce((n,x)=>n+x.skipped,0),errors:[...batchErrors,...outcomes.flatMap(x=>x.errors)]};
      setTagMessage(`Created ${r.created} sidecars; updated ${r.updated}; skipped ${r.skipped}. ${r.errors.length ? r.errors.slice(0,3).join("; ") : ""}`);
      if(r.created+r.updated>0)setTagRefreshKey(value=>value+1);
      if(!r.errors.length)setSelection([]);
    } catch(e) { setTagMessage(String(e)); } finally {setApplying(false);}
  };
  const bytes = result?.photos.reduce((n, p) => n + p.bytes, 0) || 0;
  const selectedBytes=result?.photos.filter(photo=>selection.includes(photo.path)).reduce((n,photo)=>n+photo.bytes,0)||0;
  function clearLibraryFilters(){updateSearch("");setFolderFilter("");setCollectionFilter("");setMediaType("all");setOrganizeFilter("all");setSmartFilter("");setSelectedSmartRule("");setTimelineMonth("");setMemoryYear(null);setSelectedTag("");}
  async function openSelectedVideoEditor(){
    if(!selected)return;const workspaceAtStart=activeWorkspace;setEditorError("");setEditorBusy(true);
    try{const value=await workspaceProject(activeWorkspace,{type:"open_source",path:selected.path});if(editorWorkspace.current===workspaceAtStart){setEditorInitial(value);setSelected(null);setView("editor");}}
    catch(cause){if(editorWorkspace.current===workspaceAtStart)setEditorError(String(cause));}
    finally{setEditorBusy(false);}
  }
  async function openSequenceEditor(){
    setEditorError("");
    if(!selection.length){setEditorInitial(null);setView("editor");return;}
    const media=(result?.photos||[]).filter(p=>selection.includes(p.path));
    if(media.some(p=>!["mp4","mov","m4v","mkv","webm","avi","jpg","jpeg","png","bmp","tif","tiff"].includes(p.extension.toLowerCase()))){setEditorError("Timeline imports support videos plus JPEG, PNG, BMP and TIFF photos. Other image formats remain available in the photo library.");return;}
    const workspaceAtStart=activeWorkspace;setEditorBusy(true);
    try{const value=await workspaceProject(activeWorkspace,{type:"create",paths:media.map(p=>p.path)});if(editorWorkspace.current===workspaceAtStart){setEditorInitial(value);setView("editor");}}
    catch(cause){if(editorWorkspace.current===workspaceAtStart)setEditorError(String(cause));}
    finally{setEditorBusy(false);}
  }
  return <NoticeHostSetter.Provider value={setNoticeHost}><NoticeTarget.Provider value={noticeHost}><div className="app-frame"><div className="app-notices"><ProjectSaveGuard/><VideoExportPanel/><UpdateToast updater={appUpdater} onOpenSettings={()=>setView("settings")}/>
    {storageError&&<NoticeSurface><ErrorNotice title="Your latest changes could not be saved" error={storageError}><button className="secondary-button" onClick={()=>void retrySettings().then(()=>setStorageError("")).catch(e=>setStorageError(String(e)))}>Retry saving</button></ErrorNotice></NoticeSurface>}
    </div><div className="app-shell">

    <AppSidebar items={NAV} view={view} onNavigate={next=>{if(next==="editor"){setEditorInitial(null);setEditorError("");}setView(next);}} workspaces={workspaces} workspace={activeWorkspace} onWorkspace={id=>{void openWorkspace(id);}} onCreateWorkspace={createWorkspace} theme={theme} onTheme={setTheme} version={appUpdater.currentVersion}/>


    <div className="main" ref={mainPane}>
      <main className={view==="editor"?"page page-editor":"page"}>
        {view === "library" && <>
          <PageHeading eyebrow="YOUR COLLECTION" title="Media library" description={<>Browse, search, sort, and edit the media in your current workspace.</>} className="library-heading" actions={<><button className="primary-button" onClick={()=>setView("folders")}><FolderOpen size={18}/>Manage folders</button></>}/>
          <p className="library-summary">{result?`${result.photos.length.toLocaleString()} media files · ${formatBytes(bytes)}`:"Add a folder to begin"}<span><ShieldCheck size={14}/> Originals protected</span></p>
          <section className="library-panel">

            {editorError&&<ErrorNotice title="The editor needs attention" error={editorError}/>}
            <div className="view-toolbar">
              <div className="view-switch" role="group" aria-label="Media view">
                <button className={photoView === "grid" ? "active" : ""} title="Icon grid" aria-label="Icon grid" aria-pressed={photoView === "grid"} onClick={()=>setPhotoView("grid")}><LayoutGrid size={18}/></button>
                <button className={photoView === "list" ? "active" : ""} title="List view" aria-label="List view" aria-pressed={photoView === "list"} onClick={()=>setPhotoView("list")}><List size={18}/></button>
              </div>
              <LibraryFilters><label className="sort-control">Rating <select aria-label="Organization filter" value={organizeFilter} onChange={e=>setOrganizeFilter(e.target.value as "all"|"favorites"|"rated")}><option value="all">All ratings</option><option value="favorites">Favorites</option><option value="rated">Rated</option></select></label><label className="sort-control">Media <select aria-label="Media type" value={mediaType} onChange={e=>setMediaType(e.target.value as "all"|"photo"|"video")}><option value="all">All media</option><option value="photo">Photos</option><option value="video">Videos</option></select></label>
              <label className="sort-control"><ArrowUpDown size={16}/><span>Sort:</span><select value={sortBy} aria-label="Sort media" onChange={e=>setSortBy(e.target.value as SortBy)}><option value="relevance" disabled={!query.trim()}>Relevance</option><option value="newest">Newest first</option><option value="oldest">Oldest first</option><option value="name">Name A–Z</option><option value="largest">Largest first</option><option value="smallest">Smallest first</option></select></label></LibraryFilters><button className="secondary-button" disabled={!result||!visible.length} onClick={()=>setSelection(visible.map(p=>p.path))}>Select matching</button>
<form className="search-box library-search" role="search" aria-label="Workspace media search" onSubmit={event=>{event.preventDefault();discovery.submit();}}><Search size={17}/><input aria-label="Search library" placeholder="Names, tags, people or content…" maxLength={500} value={query} onChange={e=>updateSearch(e.target.value)} disabled={!result}/>{query&&<button className="icon-button" type="button" aria-label="Clear search" title="Clear search" onClick={()=>updateSearch("")}><X size={16}/></button>}<button className="secondary-button" disabled={!result||!query.trim()||!!discovery.parsed.error} type="submit">Search</button></form>
              <button className="secondary-button" disabled={!roots.length||busy} onClick={()=>void syncFolders(roots)}>{busy?"Syncing…":"Sync"}</button>
            </div>
            {result && selection.length>0 && <div className="selection-toolbar"><span>{selection.length} selected · {formatBytes(selectedBytes)}</span><button className="secondary-button" disabled={editorBusy} onClick={()=>void openSequenceEditor()}>Create video from selection…</button><button className="secondary-button" onClick={()=>setSelection([])}>Clear selection</button><input aria-label="New XMP tags (comma separated)" placeholder="Tags separated by commas" value={tagText} onChange={e=>setTagText(e.target.value)}/><button className="primary-button" disabled={!selection.length || !tagText.trim() || applying} onClick={applyTags}>{applying?"Writing…":"Add / merge XMP tags"}</button></div>}
            {result && livePhotoPairs>0 && <p className="results-note">{livePhotoPairs} iPhone Live Photo pair{livePhotoPairs===1?"":"s"} found · {livePhotoPairing.metadataPairs} matched by Apple asset ID · {livePhotoPairing.fallbackPairs} matched by filename fallback. Original image and video files remain separate.</p>}
            {tagMessage&&<p role="status" className="results-note">{tagMessage}</p>}
            {error && <ErrorNotice title="The library could not finish this action" error={error}/>}
            {busy && <div className="empty"><div className="empty-symbol"><Images size={28}/></div><h3>Scanning your folder</h3><p>Reading file names and sizes. No photos are being changed.</p></div>}
            {!busy && !result && <div className="empty"><div className="empty-symbol"><ImageOff size={32}/></div><h3>Your library starts here</h3><p>Choose a folder containing photos or videos. Open-Photo keeps them in place and shows local previews.</p><button className="secondary-button" onClick={chooseFolder}><FolderOpen size={16}/> Add media folder</button></div>}
            {(smartFilter || selectedSmartRule) && view==="library" && <div className="selection-toolbar"><strong>Smart collection: {selectedSmartRule ? smartRules.find(rule=>rule.id===selectedSmartRule)?.name : smartFilter==="recent"?"Last 30 days":smartFilter==="thisDay"?"On this day":smartFilter==="thisMonth"?"This month in past years":smartFilter==="unrated"?"Unrated media":"Large media"}</strong><button className="secondary-button" onClick={()=>{setSmartFilter("");setSelectedSmartRule("")}}>Show all media</button></div>}
            {timelineMonth && view==="library" && <div className="selection-toolbar"><strong>Timeline month: {new Date(Number(timelineMonth.slice(0,4)),Number(timelineMonth.slice(5,7))-1,1).toLocaleDateString(undefined,{year:"numeric",month:"long"})}</strong><button className="secondary-button" onClick={()=>setTimelineMonth("")}>Show all months</button></div>}
            {memoryYear!==null && view==="library" && <div className="selection-toolbar"><strong>Memory year: {memoryYear}</strong><button className="secondary-button" onClick={()=>setMemoryYear(null)}>Show all years</button></div>}
            {selectedTag && view==="library" && <div className="selection-toolbar"><strong>Tag: {selectedTag}</strong>{tagIndexBusy&&<span role="status">Refreshing tags…</span>}{tagIndexError&&<span role="alert">{tagIndexError}</span>}<button className="secondary-button" onClick={()=>setSelectedTag("")}>Show all tags</button></div>}

            {discovery.active&&<SearchStatus busy={discovery.busy} readingTags={tagIndexBusy} tagError={tagIndexError} error={discovery.parsed.error} warnings={discovery.warnings} count={visible.length}/>}
            {!busy && result && <><div className="library-grid-bar"><span>{visible.length.toLocaleString()} items{discovery.busy?" · Updating…":""}</span>              {photoView === "grid" && <div className="zoom-control"><ZoomOut size={17}/><input aria-label="Thumbnail size" type="range" min="110" max="300" step="10" value={tileSize} onChange={e=>setTileSize(Number(e.target.value))}/><ZoomIn size={17}/><span>{tileSize}px</span></div>}</div><div className={photoView === "grid" ? "photo-grid virtual-photo-grid" : "file-list"} onScroll={e=>setScrollTop(e.currentTarget.scrollTop)} ref={setGridHost} style={photoView === "grid" ? { display:"block", padding:0, "--tile-caption-height":`${TILE_CAPTION_HEIGHT}px`, "--tile-gap-y":`${TILE_ROW_GAP}px` } as CSSProperties : undefined}>
              {photoView === "grid" ? <div style={{height:rowCount*rowHeight,position:"relative"}}><div className="virtual-rows" style={{position:"absolute",top:windowStart*rowHeight,left:24,right:24,display:"grid",gridTemplateColumns:`repeat(${columns},minmax(0,1fr))`,gridAutoRows:rowHeight,gap:0}}>
                {windowPhotos.map(p=><div className="tile-cell" key={p.path}><button className={selection.includes(p.path) ? "photo-tile is-selected" : "photo-tile"} onClick={e=>e.ctrlKey||e.metaKey ? toggleSelection(p.path):setSelected(p)}><PhotoThumb root={p.libraryRoot||root} photo={p} maxEdge={Math.max(360,tileSize*2)} hoverVideoPath={["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension.toLowerCase())?p.path:liveVideoByImage.get(p.path)?.path}/><span className="tile-caption"><span className="tile-name" title={p.name}>{p.name}</span><span className="tile-meta" title={discovery.active?discovery.match(p)?.reasons.map(reason=>reason.label).join(" · "):undefined}>{["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension.toLowerCase())?"Video":liveVideoByImage.has(p.path)?"Live image":"Image"}{photoRatings[p.path]?` · ${photoRatings[p.path]} stars`:""}</span></span></button><input type="checkbox" aria-label={`Select ${p.name}`} checked={selection.includes(p.path)} onChange={()=>toggleSelection(p.path)}/></div>)}
              </div></div> : visible.map(p=><div key={p.path} className="selectable-row"><input type="checkbox" aria-label={`Select ${p.name}`} checked={selection.includes(p.path)} onChange={()=>toggleSelection(p.path)}/><button className="file-row" onClick={()=>setSelected(p)}><div className="file-avatar"><PhotoThumb root={p.libraryRoot||root} photo={p} maxEdge={96}/></div><div className="file-name"><strong>{p.name}</strong><small>{p.extension.toUpperCase()} · {formatBytes(p.bytes)}{` · ${["mov","mp4","m4v","avi","mkv","webm"].includes(p.extension.toLowerCase())?"Video":liveVideoByImage.has(p.path)?"Live image":"Image"}`}{discovery.active?` · ${discovery.match(p)?.reasons[0]?.label||"Matches filters"}`:""}</small></div><ChevronRight size={16}/></button></div>)}
            </div>{visible.length === 0 && !discovery.busy && <div className="empty"><h3>{result.photos.length?"No matching media":"No saved media yet"}</h3><p>{result.photos.length?"Try another search or clear the active filters.":"Use Sync to read files in your folders."}</p>{result.photos.length>0&&<button className="secondary-button" onClick={clearLibraryFilters}>Clear filters</button>}</div>}</>}
          </section>
        </>}
        {view === "editor" && <><VideoSequences roots={roots} workspaceId={activeWorkspace} initial={editorInitial} onBack={()=>setView("library")}/></>}
        {view === "folders" && <><PageHeading eyebrow="LIBRARY ORGANIZATION" title="Folders & Workspaces" description={<>Choose a workspace, manage its source folders, and review indexing status in one place.</>}/><section className="folder-management-card"><div className="sync-toolbar"><strong>{roots.length} source folder{roots.length===1?"":"s"}</strong><button className="primary-button" onClick={chooseFolder} disabled={busy}>{busy?"Scanning…":"+ Add folder"}</button><button className="secondary-button" disabled={!roots.length||busy} onClick={()=>syncFolders(roots)}>{busy?"Scanning…":"Sync now"}</button>{lastSync&&<small>Last synced: {lastSync}</small>}{syncChanges&&<small>{`Changes: +${syncChanges.added} added, ${syncChanges.updated} updated, ${syncChanges.unchanged} unchanged, ${syncChanges.removed} removed${syncChanges.incomplete?" (partial scan)":""}`}</small>}</div><LibraryFolders roots={roots} onRemove={removeLibraryFolder}/></section>{error&&<ErrorNotice title="The folder action could not finish" error={error}/>}<h2>Library cleanup</h2>{result && <div className="duplicate-bar"><button className="secondary-button" disabled={duplicateBusy||!result||!roots.length} onClick={inspectDuplicates}>Find Exact Duplicates</button><button className="secondary-button" disabled={duplicateBusy||!result||!roots.length} onClick={inspectSimilar}>Find Similar Photos</button>{duplicateBusy&&<span>Analyzing local files…</span>}{duplicates.length>0&&<span>{duplicates.length} identical-file groups</span>}{similarGroups.length>0&&<span>{similarGroups.length} visually similar groups</span>}{[...duplicates.map(group=>({paths:group.paths,label:"Identical files"})),...similarGroups.map(group=>({paths:group.paths,label:"Visually similar photos"}))].map((group,i)=><div className="duplicate-group" key={i}><strong>{group.label} · {group.paths.length} files</strong><p>Inspect each candidate and optionally mark one to keep. This review never deletes or moves a file.</p>{group.paths.map(path=><div key={path} className="collection-row"><small className="break-anywhere">{duplicateReview[group.paths.join("|")]===path?"✓ Preferred copy · ":""}{path}</small><button className="secondary-button" onClick={()=>{const candidate=result?.photos.find(photo=>photo.path===path);if(candidate)setSelected(candidate);else setError("This file is not in the current library view.")}}>Preview</button><button className="secondary-button" aria-pressed={duplicateReview[group.paths.join("|")]===path} onClick={()=>setDuplicateReview(previous=>({...previous,[group.paths.join("|")]:path}))}>Mark to keep</button></div>)}</div>)}</div>}<div className="browse-heading"><h2>Browse folders</h2><BrowseView label="Folder view" value={folderView} onChange={setFolderView}/></div><p>Choose a folder to filter the Media library without moving any files.</p><div className={`folder-list browse-items ${folderView}`}>{folders.map(folder=><button key={folder} className="folder-item" onClick={()=>{setFolderFilter(folder==="(root)"?"":folder);setView("library")}}><FolderOpen size={20}/>{folder}<ChevronRight size={16}/></button>)}{folders.length===0&&<p>Open a workspace folder to browse its subfolders.</p>}</div></>}
        {view === "map" && <><PageHeading eyebrow="PHOTO LOCATIONS" title="Map" description={<>Zoom and explore GPS-tagged photos from your current workspace. Select a marker to preview its photo.</>}/>{error&&<div role="alert" className="error">{error}</div>}{geoBusy?<div className="results-note" role="status">Reading embedded GPS locations…</div>:<div className="results-note">{geoPoints.length} photo location{geoPoints.length===1?"":"s"} found · JPEG/TIFF GPS is supported today; HEIC/MOV location metadata is planned.</div>}{root&&result?<PhotoMap points={geoPoints} onChoose={path=>{const photo=result.photos.find(p=>p.path===path);if(photo)setSelected(photo)}}/>:<div className="placeholder-panel"><div className="empty-symbol"><MapPin size={30}/></div><h2>No map data yet</h2><p>Open a workspace and scan a media folder to discover GPS-tagged photos.</p></div>}</>}
        {view === "collections" && <PageHeading eyebrow="VIRTUAL ORGANIZATION" title="Collections" description={<>Group and filter media without moving or duplicating the original files.</>} actions={<BrowseView label="Collection view" value={collectionView} onChange={setCollectionView}/>}/>}
        {view === "collections" && <><h2>Smart collections</h2><p>Dynamic, read-only views prefer embedded capture dates when available and fall back to file modified dates. Nothing is moved or duplicated.</p><div className={`folder-list browse-items ${collectionView}`}>{([{id:"recent",name:"Captured in the last 30 days"},{id:"thisDay",name:"On this day in previous years"},{id:"large",name:"Large media (50 MB or more)"},{id:"unrated",name:"Media not yet rated"},{id:"thisMonth",name:"This month in previous years"}] as const).map(item=><button className="folder-item" key={item.id} onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth("");setSmartFilter(item.id);setSelectedSmartRule("");setCollectionFilter("");setQuery("");setView("library")}}><Images size={18}/>{item.name}<ChevronRight size={16}/></button>)}</div></>}
        {view === "collections" && <><h2>Your rules</h2><p>Create a reusable, read-only filter. Each rule belongs to this workspace.</p><button className="secondary-button" disabled={!activeWorkspace} onClick={()=>setForm("smart")}>Create smart collection</button><div className={`folder-list browse-items ${collectionView}`}>{smartRules.filter(rule=>rule.workspaceId===activeWorkspace).map(rule=><div key={rule.id} className="collection-row"><button className="folder-item" onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth("");setSmartFilter("");setSelectedSmartRule(rule.id);setCollectionFilter("");setView("library")}}><Images size={18}/>{rule.name} · {rule.type} · {rule.minRating}+ stars · {rule.minMB}+ MB</button><button className="destructive-button" title="Remove this smart collection rule" onClick={()=>{setSmartRules(old=>old.filter(item=>item.id!==rule.id));if(selectedSmartRule===rule.id)setSelectedSmartRule("")}}>Remove</button></div>)}</div></>}
        {view === "collections" && <><h2>Manual collections</h2><p>Save a reusable group from the media you select in Media library.</p><button className="primary-button" disabled={!activeWorkspace} onClick={()=>setForm("collection")}>Create collection {selection.length? `with ${selection.length} selected` : ""}</button><div className={`folder-list browse-items ${collectionView}`}>{collections.filter(c=>c.workspaceId===activeWorkspace).map(c=><div key={c.id} className="collection-row"><button className="folder-item" onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth("");setCollectionFilter(c.id);setView("library")}}><Images size={18}/>{c.name} · {c.paths.length} items</button><button className="destructive-button" title="Remove this collection without deleting media" onClick={()=>setCollections(old=>old.filter(x=>x.id!==c.id))}>Remove</button></div>)}</div></>}
        {view === "timeline" && <><PageHeading eyebrow="BY CAPTURE DATE" title="By date" description={<>Select a month to open only media from that capture month. EXIF still-image dates and QuickTime MOV/MP4/M4V dates are preferred; file modified time is the fallback.</>}/><div className="folder-list">{[...new Set([...(result?.photos||[])].sort((a,b)=>photoTime(b)-photoTime(a)).map(photoMonthKey))].map(monthKey=>{const [year,month]=monthKey.split("-").map(Number);const label=new Date(year,month-1,1).toLocaleDateString(undefined,{year:"numeric",month:"long"});return <button className="folder-item" key={monthKey} onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth(monthKey);setSmartFilter("");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setSortBy("newest");setView("library")}}>{label}</button>})}</div></>}
        {view === "memories" && <><PageHeading eyebrow="ON THIS DAY" title="Memories" description={<>Revisit photos and videos captured on {new Date().toLocaleDateString(undefined,{month:"long",day:"numeric"})} in previous years. Embedded EXIF and QuickTime dates are preferred, with file modified time used only as a fallback.</>}/><div className="selection-toolbar"><strong>{onThisDayMemories.reduce((count,group)=>count+group.photos.length,0)} memories across {onThisDayMemories.length} past years</strong><button className="primary-button" disabled={!onThisDayMemories.length} onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth("");setSmartFilter("thisDay");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setSortBy("newest");setView("library")}}>Open all On This Day</button><button className="secondary-button" onClick={()=>{setSelectedTag("");setMemoryYear(null);setTimelineMonth("");setSmartFilter("thisMonth");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setSortBy("newest");setView("library")}}>This month in past years</button></div>{onThisDayMemories.length===0?<div className="placeholder-panel"><div className="empty-symbol"><Images size={30}/></div><h2>No memories for today yet</h2><p>Open-Photo will show past-year matches here when your indexed library contains media captured on today’s month and day.</p></div>:<div className="folder-list">{onThisDayMemories.map(group=><div className="settings-card" key={group.year}><div><h2>{new Date().getFullYear()-group.year} years ago · {group.year}</h2><p>{group.photos.length} item{group.photos.length===1?"":"s"} from this day.</p><button className="secondary-button" onClick={()=>{setMemoryYear(group.year);setTimelineMonth("");setSmartFilter("");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setSortBy("newest");setView("library")}}>Browse all media from {group.year}</button></div><div className="folder-list">{group.photos.slice(0,12).map(photo=><button className="folder-item" key={photo.path} onClick={()=>setSelected(photo)}><Images size={18}/>{photo.name} · {photoDate(photo).toLocaleTimeString(undefined,{hour:"numeric",minute:"2-digit"})}</button>)}</div>{group.photos.length>12&&<small>Showing the first 12 of {group.photos.length} memories from {group.year}.</small>}</div>)}</div>}<h2>All years</h2><p>Jump into any capture year in the current workspace, even when there are no On This Day matches.</p><div className="folder-list">{memoryYears.map(item=><button className="folder-item" key={item.year} onClick={()=>{setMemoryYear(item.year);setTimelineMonth("");setSmartFilter("");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setSortBy("newest");setView("library")}}><Images size={18}/><span>{item.year} · {item.count.toLocaleString()} item{item.count===1?"":"s"}<small>{item.exif.toLocaleString()} EXIF · {item.quicktime.toLocaleString()} QuickTime · {item.fallback.toLocaleString()} modified fallback</small></span><ChevronRight size={16}/></button>)}{memoryYears.length===0&&<p>Open a workspace and scan media to browse memories by year.</p>}</div></>}
        {view === "people" && <PeopleView aiOptIn={aiOptIn} ready={peopleReady} status={peopleStatus} busy={peopleBusy} hasLibrary={!!result} message={peopleMessage} groups={peopleGroups} names={peopleNames} onOpenSettings={()=>setView("settings")} onIndex={()=>void indexPeopleWorkspace()} onRefresh={()=>void refreshPeople()} onName={(id,name)=>invoke("ai_name_person",{id,name}).then(()=>{setPeopleNames(previous=>({...previous,[id]:name}));setPeopleGroups(previous=>previous.map(group=>group.id===id?{...group,name}:group))}).catch(e=>{setPeopleMessage(String(e));throw e;})} onOpenPhoto={path=>{const candidate=result?.photos.find(photo=>photo.path===path);if(candidate)setSelected(candidate)}}/>}
        {view === "tags" && <><PageHeading eyebrow="PORTABLE METADATA" title="Tags" description={<>Browse keywords stored in Open-Photo-managed XMP sidecars. Use / to build hierarchy, for example People/Family/Marc or Places/Florida/Tampa.</>}/>{tagIndexBusy&&<div className="results-note" role="status">Reading managed XMP sidecars…</div>}{tagIndexError&&<div role="alert" className="error">{tagIndexError}</div>}<div className="selection-toolbar"><strong>{tagBranches.length} tag branch{tagBranches.length===1?"":"es"}</strong><button className="secondary-button" disabled={!result||tagIndexBusy} onClick={()=>setTagRefreshKey(value=>value+1)}>Refresh tags</button></div><div className="folder-list">{tagBranches.map(item=><button className="folder-item" key={item.tag} style={{paddingLeft:Math.min(48,12+(item.depth-1)*18)}} onClick={()=>{setSelectedTag(item.tag);setMemoryYear(null);setTimelineMonth("");setSmartFilter("");setSelectedSmartRule("");setCollectionFilter("");setQuery("");setView("library")}}><Tags size={18}/><span>{item.tag.split("/").at(-1)}<small>{item.tag} · {item.count} item{item.count===1?"":"s"}</small></span><ChevronRight size={16}/></button>)}{!tagIndexBusy&&tagBranches.length===0&&<div className="placeholder-panel"><div className="empty-symbol"><Tags size={30}/></div><h2>No Open-Photo tags yet</h2><p>Select media in the Library and use Add / merge XMP tags. Hierarchical tags use / between levels.</p></div>}</div></>}
        {view === "settings" && <div className="settings-view"><PageHeading eyebrow="PREFERENCES" title="Settings" description={<>Manage appearance, local AI, backups, updates, and library health.</>}/><div className="settings-card"><div><h2>Accent color</h2><p>Choose the app accent used for selected items, buttons, focus states, and highlights.</p></div><div className="accent-picker" role="group" aria-label="Accent color">{ACCENTS.map(item=><button key={item.id} className={accent===item.id?"accent-swatch selected":"accent-swatch"} aria-label={item.label+" accent"} aria-pressed={accent===item.id} title={item.label} onClick={()=>setAccent(item.id)}><span style={{background:item.color}}/><small>{item.label}</small></button>)}</div></div><UpdateSettingsCard updater={appUpdater}/><div className="settings-card"><div><h2>Library metadata backup</h2><p>Save ratings, collections, workspace settings, and saved person names to a separate JSON backup.</p></div><div><button className="secondary-button" onClick={()=>void exportMetadataBackup()}>Export metadata backup</button>{backupMessage&&<p className="results-note" role="status">{backupMessage}</p>}</div></div><LocalAiSettings enabled={aiOptIn} onChange={setAiEnabled} models={aiModels} busy={aiModelBusy} message={aiModelMessage} onRefresh={()=>void refreshModels()} onCheck={()=>void runAiSelfCheck()} checking={aiValidationBusy} runtime={aiRuntime} device={aiDevice} onDeviceChange={setAiDevice}/><section aria-label="Local content indexes"><h2>Local content indexes</h2>
            {aiOptIn&&result&&<div className="selection-toolbar"><strong>Local content indexes</strong><button className="secondary-button" disabled={!semanticReady||aiIndexBusy} onClick={()=>void indexAiPhotos(result.photos,"library")}>{aiIndexBusy?"Indexing…":"Index library with AI"}</button>{aiIndexBusy&&<button className="secondary-button" onClick={()=>{aiCancelRequested.current=true;setAiIndexMessage(`Cancelling after the current small batch… ${aiIndexProgress.done}/${aiIndexProgress.total}`)}}>Cancel AI indexing</button>}<button className="secondary-button" disabled={!ocrReady||ocrBusy} onClick={()=>void indexOcrWorkspace()}>{ocrBusy?"Working…":"Index library text"}</button>{selection.length>0&&<button className="secondary-button" disabled={!semanticReady||aiIndexBusy} onClick={()=>void indexAiPhotos(result.photos.filter(photo=>selection.includes(photo.path)),"selected")}>Index selected with AI</button>}{aiIndexMessage&&<span role="status">{aiIndexMessage}</span>}{ocrMessage&&<span role="status">{ocrMessage}</span>}</div>}</section><div className="settings-card"><div><h2>Workspace backup</h2><p>Export selected workspace settings, folder references and virtual collections to JSON. Photo and video files are not copied.</p></div><button className="secondary-button" disabled={!activeWorkspace} onClick={exportWorkspace}>Export Workspace JSON</button></div><div className="settings-card"><div><h2>Library health check</h2><p>Inspect indexed files for missing, inaccessible, or modified items. This is read-only and never removes your media or index records.</p>{healthReport&&<p>{healthReport.indexed} indexed · {healthReport.available} available · {healthReport.missing} missing · {healthReport.inaccessible} inaccessible · {healthReport.modified} changed since indexing</p>}{healthError&&<p role="alert" className="error">{healthError}</p>}</div><button className="secondary-button" disabled={healthBusy||!root} onClick={()=>{setHealthBusy(true);setHealthError("");invoke<{indexed:number;available:number;missing:number;inaccessible:number;modified:number}>("library_health",{root}).then(setHealthReport).catch(e=>setHealthError(String(e))).finally(()=>setHealthBusy(false))}}>{healthBusy?"Checking…":"Check library health"}</button></div><div className="settings-card"><div><h2>File safety</h2><p>Original media stays read-only. Adding tags can create sidecars or merge keywords into Open-Photo-managed XMP files. Foreign sidecars are skipped. Edits and exports save separate copies; original photos and videos are never deleted or overwritten.</p></div><span className="safety-pill">Protected</span></div></div>}
      </main>
    </div>
    {form&&<TaskForm title={form==="workspace"?"Create workspace":form==="smart"?"Create smart collection":"Create collection"} submitLabel="Create" onClose={()=>setForm(null)} fields={form==="smart"?[
      {name:"name",label:"Collection name"},{name:"type",label:"Media type",value:"all",options:[{value:"all",label:"All media"},{value:"photo",label:"Photos"},{value:"video",label:"Videos"}]},{name:"rating",label:"Minimum star rating",type:"number",value:"0",min:0,max:5},{name:"size",label:"Minimum file size (MB)",type:"number",value:"0",min:0,step:"any"}
    ]:[{name:"name",label:form==="workspace"?"Workspace name":"Collection name"}]} onSubmit={values=>{
      if(form==="workspace")createWorkspace(values.name);
      else if(form==="collection")setCollections(old=>[...old,{id:crypto.randomUUID(),workspaceId:activeWorkspace,name:values.name,paths:[...selection]}]);
      else {const minRating=Number(values.rating),minMB=Number(values.size);if(!Number.isInteger(minRating)||minRating<0||minRating>5||!Number.isFinite(minMB)||minMB<0)throw Error("Choose a rating from 0 to 5 and a file size of zero or greater.");setSmartRules(old=>[...old,{id:crypto.randomUUID(),workspaceId:activeWorkspace,name:values.name,type:values.type as "all"|"photo"|"video",minRating,minMB}]);}
    }}/>}
    {selected && <Modal label="File details" onClose={()=>{setSelected(null);setFullScreenPreview(false);}} className={fullScreenPreview?"details details-fullscreen":"details"}>{editorError&&<ErrorNotice title="The editor could not open" error={editorError}/>}<MediaDetailsHeader video={["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension.toLowerCase())} editing={editorBusy} fullscreen={fullScreenPreview} canPrevious={visible.findIndex(p=>p.path===selected.path)>0} canNext={visible.findIndex(p=>p.path===selected.path)>=0&&visible.findIndex(p=>p.path===selected.path)<visible.length-1} onEdit={()=>void openSelectedVideoEditor()} onFullscreen={()=>setFullScreenPreview(value=>!value)} onPrevious={()=>{const index=visible.findIndex(p=>p.path===selected.path);if(index>0)setSelected(visible[index-1]);}} onNext={()=>{const index=visible.findIndex(p=>p.path===selected.path);if(index>=0&&index<visible.length-1)setSelected(visible[index+1]);}} onClose={()=>setSelected(null)}/>{discovery.active&&discovery.match(selected)&&<div className="search-match-details"><strong>Why this matched</strong><ul>{discovery.match(selected)!.reasons.map(reason=><li key={reason.kind}>{reason.label}</li>)}</ul></div>}<div className="file-summary"><strong>{selected.name}</strong><span>{selected.extension.toUpperCase()} · {formatBytes(selected.bytes)}</span><span>{photoDate(selected).toLocaleString()} · {photoDateSource(selected)}</span></div><div className={`detail-preview edit-preview-shell${["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)?" detail-preview-video":""}`}>{["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)?<IPhoneMotion autoLoad root={selected.libraryRoot||root} path={selected.path} onOpenExternal={()=>void invoke("open_video",{root:selected.libraryRoot||root,path:selected.path}).catch(e=>setError(String(e)))}/>:editShowOriginal?<PhotoThumb root={selected.libraryRoot||root} photo={selected} maxEdge={1600}/>:editPreviewSrc?<img className="photo-image edit-preview-image" src={editPreviewSrc} alt={`Edited preview of ${selected.name}`}/>:<PhotoThumb root={selected.libraryRoot||root} photo={selected} maxEdge={1600}/>} {editPreviewBusy&&!editShowOriginal&&<span className="edit-preview-status">Updating preview…</span>}{!["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)&&<button className="edit-compare-button" onClick={()=>setEditShowOriginal(value=>!value)}>{editShowOriginal?"Show edits":"Show original"}</button>}</div>{liveVideoByImage.has(selected.path)&&<div className="live-photo-motion"><strong>Live Photo · motion clip</strong><IPhoneMotion root={selected.libraryRoot||root} path={liveVideoByImage.get(selected.path)!.path} onOpenExternal={()=>void invoke("open_video",{root:selected.libraryRoot||root,path:liveVideoByImage.get(selected.path)!.path}).catch(e=>setError(String(e)))}/></div>}{["mov","mp4","m4v","avi","mkv","webm"].includes(selected.extension)&&<button className="primary-button" onClick={()=>invoke("open_video",{root:selected.libraryRoot||root,path:selected.path}).catch(e=>setError(String(e)))}>Open in Windows player</button>}{EDITABLE_IMAGE_EXTENSIONS.has(selected.extension.toLowerCase())&&<div className="settings-card edit-card"><div><h2>Edit & Export</h2><p>Preview adjustments live, then save a new copy. The original file is never overwritten.</p><div className="selection-toolbar"><button className="secondary-button" onClick={()=>setEditRotation(value=>(value===270?0:(value+90) as 0|90|180|270))}>Rotate 90° · {editRotation}°</button><button className="secondary-button" aria-pressed={editFlipH} onClick={()=>setEditFlipH(value=>!value)}>Flip horizontal</button><button className="secondary-button" aria-pressed={editFlipV} onClick={()=>setEditFlipV(value=>!value)}>Flip vertical</button><button className="secondary-button" onClick={()=>{setEditRotation(0);setEditFlipH(false);setEditFlipV(false);setEditBrightness(0);setEditContrast(0);setEditCropLeft(0);setEditCropTop(0);setEditCropRight(0);setEditCropBottom(0);setEditResizeWidth("");setEditResizeHeight("");setEditGrayscale(false);setEditHue(0);setEditBlur(0);setEditSharpen(0);setEditSharpenThreshold(0);setEditJpegQuality(92);setEditPreserveModifiedTime(true);setEditShowOriginal(false)}}>Reset edits</button></div><div className="edit-crop-presets"><strong>Crop preset</strong><div className="selection-toolbar"><button className="secondary-button" onClick={()=>applyCropPreset()}>Original / Free</button><button className="secondary-button" onClick={()=>applyCropPreset(1)}>1:1</button><button className="secondary-button" onClick={()=>applyCropPreset(4/3)}>4:3</button><button className="secondary-button" onClick={()=>applyCropPreset(16/9)}>16:9</button></div></div><div className="edit-controls-grid"><label>Brightness <strong>{editBrightness}</strong><input type="range" min="-100" max="100" value={editBrightness} onChange={e=>setEditBrightness(Number(e.target.value))}/></label><label>Contrast <strong>{editContrast}</strong><input type="range" min="-100" max="100" value={editContrast} onChange={e=>setEditContrast(Number(e.target.value))}/></label><label>Hue <strong>{editHue}°</strong><input type="range" min="-180" max="180" value={editHue} onChange={e=>setEditHue(Number(e.target.value))}/></label><label>Blur <strong>{(editBlur/10).toFixed(1)}</strong><input type="range" min="0" max="100" value={editBlur} onChange={e=>setEditBlur(Number(e.target.value))}/></label><label>Sharpen <strong>{(editSharpen/10).toFixed(1)}</strong><input type="range" min="0" max="100" value={editSharpen} onChange={e=>setEditSharpen(Number(e.target.value))}/></label><label>Sharpen threshold <strong>{editSharpenThreshold}</strong><input type="range" min="0" max="100" value={editSharpenThreshold} onChange={e=>setEditSharpenThreshold(Number(e.target.value))}/></label><label>Grayscale <strong>{editGrayscale?"On":"Off"}</strong><input type="checkbox" checked={editGrayscale} onChange={e=>setEditGrayscale(e.target.checked)}/></label><label>Crop left <strong>{editCropLeft}%</strong><input type="range" min="0" max="45" step="0.1" value={editCropLeft} onChange={e=>setEditCropLeft(Number(e.target.value))}/></label><label>Crop right <strong>{editCropRight}%</strong><input type="range" min="0" max="45" step="0.1" value={editCropRight} onChange={e=>setEditCropRight(Number(e.target.value))}/></label><label>Crop top <strong>{editCropTop}%</strong><input type="range" min="0" max="45" step="0.1" value={editCropTop} onChange={e=>setEditCropTop(Number(e.target.value))}/></label><label>Crop bottom <strong>{editCropBottom}%</strong><input type="range" min="0" max="45" step="0.1" value={editCropBottom} onChange={e=>setEditCropBottom(Number(e.target.value))}/></label></div><div className="edit-resize-row"><label>Resize width (px)<input type="number" min="1" max="20000" placeholder="Keep original" value={editResizeWidth} onChange={e=>setEditResizeWidth(e.target.value)}/></label><label>Resize height (px)<input type="number" min="1" max="20000" placeholder="Auto / original" value={editResizeHeight} onChange={e=>setEditResizeHeight(e.target.value)}/></label></div><label className="edit-quality-control">JPEG export quality <strong>{editJpegQuality}</strong><input type="range" min="40" max="100" value={editJpegQuality} onChange={e=>setEditJpegQuality(Number(e.target.value))}/></label><label className="edit-toggle-control"><input type="checkbox" checked={editPreserveModifiedTime} onChange={e=>setEditPreserveModifiedTime(e.target.checked)}/><span><strong>Preserve source modified date</strong><small>Keeps the source file date on the new edited copy. Turn this off to use the export time instead.</small></span></label>{editOutputDimensions&&<div className="edit-output-size"><strong>Output size</strong><span>{editOutputDimensions.width.toLocaleString()} × {editOutputDimensions.height.toLocaleString()} px · {((editOutputDimensions.width*editOutputDimensions.height)/1_000_000).toFixed(1)} MP</span></div>}<small>Resize values must be whole numbers from 1–20,000. Enter only width or height to preserve aspect ratio. JPEG quality applies only when the exported filename ends in .jpg or .jpeg. Edited output is limited to 100 megapixels.</small>{editPreviewError&&<p className="error" role="alert">{editPreviewError}</p>}{editExportMessage&&<p className="results-note" role="status">{editExportMessage}</p>}</div><button className="primary-button" disabled={editExportBusy||!!editPreviewError} onClick={()=>void exportEditedPhoto()}>{editExportBusy?"Exporting…":"Save edited copy"}</button></div>}{aiOptIn&&<div className="rating-control"><strong>Local AI</strong><button className="secondary-button" disabled={!semanticReady} onClick={()=>{setAiIndexMessage("Indexing locally…");void invoke<number>("ai_index_semantic_image",{root:selected.libraryRoot||root,path:selected.path,device:aiDevice}).then(dimensions=>setAiIndexMessage(`Stored ${dimensions}-value image embedding locally.`)).catch(e=>setAiIndexMessage(String(e)))}}>Index this photo</button><button className="secondary-button" disabled={!semanticReady} onClick={()=>{setAiIndexMessage("Searching indexed photos locally…");setAiSimilar([]);void invoke<{path:string;score:number}[]>("ai_find_similar_by_photo",{root:selected.libraryRoot||root,path:selected.path,limit:12}).then(matches=>{setAiSimilar(matches);setAiIndexMessage(`Found ${matches.length} indexed semantic matches.`)}).catch(e=>setAiIndexMessage(String(e)))}}>Find AI-similar photos</button>{aiIndexMessage&&<span>{aiIndexMessage}</span>}{aiSimilar.length>0&&<div className="folder-list">{aiSimilar.map(match=><button className="folder-item" key={match.path} onClick={()=>{const candidate=result?.photos.find(photo=>photo.path===match.path);if(candidate)setSelected(candidate)}}>{Math.round(match.score*100)}% · {match.path}</button>)}</div>}</div>}<div className="rating-control" aria-label="Photo rating"><strong>Rating</strong> {[1,2,3,4,5].map(star=><button key={star} className="secondary-button" aria-label={`Rate ${star} stars`} aria-pressed={photoRatings[selected.path]===star} onClick={()=>ratePhoto(selected.path,star)}>{(photoRatings[selected.path]||0)>=star?"★":"☆"}</button>)}<button className="secondary-button" onClick={()=>ratePhoto(selected.path,0)}>Clear rating</button></div><dl><dt>Name</dt><dd>{selected.name}</dd><dt>Location</dt><dd className="break-anywhere">{selected.path}</dd><dt>File size</dt><dd>{formatBytes(selected.bytes)}</dd><dt>File format</dt><dd>{selected.extension.toUpperCase()}</dd><dt>Date used</dt><dd>{photoDate(selected).toLocaleString()}</dd><dt>Date source</dt><dd>{photoDateSource(selected)}</dd>{selected.capture_local&&<><dt>Embedded capture</dt><dd>{new Date(selected.capture_local).toLocaleString()}</dd></>}<dt>File modified</dt><dd>{selected.modified_unix ? new Date(selected.modified_unix*1000).toLocaleString() : "Unknown"}</dd></dl><FileLocation root={selected.libraryRoot||root} path={selected.path} extension={selected.extension} modified={selected.modified_unix} desktop={isDesktop}/><div className="details-foot"><ShieldCheck size={16}/> File opened read-only</div></Modal>}
  </div></div></NoticeTarget.Provider></NoticeHostSetter.Provider>;
}

function PeopleView({aiOptIn,ready,status,busy,hasLibrary,message,groups,names,onOpenSettings,onIndex,onRefresh,onName,onOpenPhoto}:{aiOptIn:boolean;ready:boolean;status:FaceIndexStatus|null;busy:boolean;hasLibrary:boolean;message:string;groups:PersonGroup[];names:Record<string,string>;onOpenSettings:()=>void;onIndex:()=>void;onRefresh:()=>void;onName:(id:string,name:string)=>Promise<void>;onOpenPhoto:(path:string)=>void}) {
  const [naming,setNaming]=useState<PersonGroup|null>(null);
  if(!aiOptIn)return <><PageHeading eyebrow="LOCAL PEOPLE INDEX" title="People" description={<>Organize familiar faces without sending photos to a cloud recognition service.</>}/><div className="placeholder-panel"><h2>Local AI is off</h2><p>Enable Local AI in Settings before indexing faces.</p><button className="secondary-button" onClick={onOpenSettings}>Open AI settings</button></div></>;
  if(!ready)return <><PageHeading eyebrow="LOCAL PEOPLE INDEX" title="People" description={<>Find faces and organize suggested groups locally.</>}/><div className="placeholder-panel"><h2>People models are not ready</h2><p>Check the included AI files in Settings. Repair or reinstall Open-Photo if files are missing.</p><button className="secondary-button" onClick={onOpenSettings}>Open AI settings</button></div></>;
  return <><PageHeading eyebrow="LOCAL PEOPLE INDEX" title="People" description={<>Find faces and organize suggested groups locally. Suggested groups stay on this computer and original photos remain read-only.</>}/><div className="selection-toolbar"><strong>{status?`${status.faces} faces across ${status.indexed_images} indexed photos`:"People index not loaded"}</strong><button className="primary-button" disabled={busy||!hasLibrary} onClick={onIndex}>{busy?"Indexing faces…":"Index People"}</button><button className="secondary-button" disabled={busy} onClick={onRefresh}>Refresh groups</button></div>{message&&<div className="results-note" role="status">{message}</div>}{groups.length===0&&!busy&&<div className="placeholder-panel"><div className="empty-symbol"><Users size={30}/></div><h2>No suggested people groups yet</h2><p>Turn on Local AI in Settings, then choose Index People. The models are included with the app.</p></div>}<div className="folder-list">{groups.map((group,index)=><div className="settings-card" key={group.id}><div><h2>{group.name||names[group.id]||names[group.legacy_id||""]||`Suggested person ${index+1}`}</h2><p>{group.members.length} face{group.members.length===1?"":"s"} in this local group.</p></div><div><button className="secondary-button" onClick={()=>setNaming(group)}>Name person</button><div className="folder-list">{group.members.slice(0,12).map(member=><button className="folder-item" key={`${member.path}:${member.face_index}`} onClick={()=>onOpenPhoto(member.path)}><Users size={16}/>{member.path.split(/[\\/]/).pop()} · {Math.round(member.score*100)}% face confidence</button>)}</div>{group.members.length>12&&<small>Showing the first 12 of {group.members.length} faces.</small>}</div></div>)}</div>{naming&&<TaskForm title="Name person" submitLabel="Save name" fields={[{name:"name",label:"Person name (leave blank to remove)",required:false,value:naming.name||names[naming.id]||names[naming.legacy_id||""]||""}]} onClose={()=>setNaming(null)} onSubmit={values=>onName(naming.id,values.name)}/>}</>;
}

function Placeholder({title,subtitle,icon:Icon,description}:{title:string;subtitle:string;icon:typeof Images;description:string}) {
  return <><PageHeading eyebrow="COMING SOON" title={title} description={<>{subtitle}</>}/><div className="placeholder-panel"><div className="empty-symbol"><Icon size={30}/></div><h2>Built for the long term</h2><p>{description}</p></div></>;
}
function formatBytes(n:number){if(n<1024)return n+" B";const u=["KB","MB","GB","TB"];let i=-1;do{n/=1024;i++}while(n>=1024&&i<u.length-1);return n.toFixed(n<10?1:0)+" "+u[i];}

function PhotoThumb({root,photo,maxEdge,hoverVideoPath}:{root:string;photo:Photo;maxEdge:number;hoverVideoPath?:string}) {
  const [src,setSrc] = useState("");
  const [failed,setFailed] = useState(false);
  const [hovering,setHovering]=useState(false),[hoverSrc,setHoverSrc]=useState("");
  useEffect(() => {
    let active = true;
    setSrc(""); setFailed(false);
    if (!isDesktop || !root || ["heic","heif"].includes(photo.extension)) return;
    invoke<string>(["mov","mp4","m4v","avi","mkv","webm"].includes(photo.extension.toLowerCase())?"video_poster":"preview_image",{root,path:photo.path,maxEdge})
      .then(data=>{if(active)setSrc(data);})
      .catch(()=>{if(active)setFailed(true);});
    return ()=>{active=false;};
  },[root,photo.path,maxEdge]);
  useEffect(()=>{
    let active=true;
    if(!hovering||!hoverVideoPath||!isDesktop||!root){setHoverSrc("");return()=>{active=false};}
    void invoke<string>("video_source",{root,path:hoverVideoPath}).then(path=>{if(active)setHoverSrc(convertFileSrc(path));}).catch(()=>{if(active)setHoverSrc("");});
    return()=>{active=false};
  },[hovering,hoverVideoPath,root]);
  const still=["heic","heif"].includes(photo.extension)?<IPhonePicture root={root} path={photo.path} name={photo.name}/>:src?<img className="photo-image" src={src} alt={photo.name} loading="lazy"/>:<div className="photo-fallback"><Images size={28}/><small>{failed ? "Preview unavailable" : "Loading preview…"}</small></div>;
  if(!hoverVideoPath)return still;
  return <span className="library-hover-preview" onPointerEnter={()=>setHovering(true)} onPointerLeave={()=>setHovering(false)}>{still}{hovering&&hoverSrc&&<video className="library-hover-video" src={hoverSrc} muted loop autoPlay playsInline preload="metadata" aria-label={"Preview "+photo.name}/>}</span>;
}
