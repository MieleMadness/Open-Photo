import {invoke} from "@tauri-apps/api/core";
import {Update} from "@tauri-apps/plugin-updater";

// Keep Tauri's resource, download, signature-verification, and install implementation.
// The native check only configures connection/read deadlines for large installers.
export async function checkForAppUpdate():Promise<Update|null>{
  const metadata=await invoke<ConstructorParameters<typeof Update>[0]|null>("check_app_update");
  return metadata?new Update(metadata):null;
}
