export type Workspace={id:string;name:string;root:string;roots?:string[]};
export function workspaceFolders(workspace:Workspace|undefined):string[]{
  return workspace?(workspace.roots?.length?workspace.roots:workspace.root?[workspace.root]:[]):[];
}
export function initialWorkspace(workspaces:Workspace[],active:string):Workspace|undefined{
  return workspaces.find(item=>item.id===active)||workspaces[0];
}
