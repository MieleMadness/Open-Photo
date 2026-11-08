export type FolderSelection={root:string;directory:string};
type RootedMedia={path:string;libraryRoot?:string};

export function normalizeLibraryPath(path:string){
  return path.replace(/\\/g,"/").replace(/^\/\/\?\/UNC\//i,"//").replace(/^\/\/\?\//,"").replace(/\/+$/g,"").toLowerCase();
}
function folderFor(media:RootedMedia,fallbackRoot:string):FolderSelection|null{
  const root=media.libraryRoot||fallbackRoot,path=media.path.replace(/\\/g,"/"),parent=path.slice(0,path.lastIndexOf("/"));
  const normalizedRoot=normalizeLibraryPath(root),normalizedParent=normalizeLibraryPath(parent);
  if(normalizedParent===normalizedRoot)return {root,directory:""};
  if(!normalizedParent.startsWith(normalizedRoot+"/"))return null;
  const displayParent=parent.replace(/^\/\/\?\/UNC\//i,"//").replace(/^\/\/\?\//,"");
  return {root,directory:displayParent.split("/").slice(normalizedRoot.split("/").length).join("/")};
}
export function folderIdentity(folder:FolderSelection){return JSON.stringify([normalizeLibraryPath(folder.root),normalizeLibraryPath(folder.directory)]);}
/** A folder row selects direct children; all-folders is represented separately by null. */
export function matchesLibraryFolder(media:RootedMedia,fallbackRoot:string,selection:FolderSelection|null){
  if(!selection)return true;
  const actual=folderFor(media,fallbackRoot);
  return !!actual&&folderIdentity(actual)===folderIdentity(selection);
}
export function libraryFolderChoices(media:RootedMedia[],fallbackRoot:string){
  const folders=new Map<string,FolderSelection>();
  for(const item of media){const folder=folderFor(item,fallbackRoot);if(folder)folders.set(folderIdentity(folder),folder);}
  return [...folders.values()].sort((a,b)=>normalizeLibraryPath(a.root).localeCompare(normalizeLibraryPath(b.root))||a.directory.localeCompare(b.directory));
}
export function folderLabel(folder:FolderSelection,multipleRoots:boolean){
  const directory=folder.directory||"(root)";
  return multipleRoots?`${directory} · ${folder.root}`:directory;
}
