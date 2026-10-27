import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

function decodeBase64(encoded: string): Uint8Array {
  const raw = atob(encoded);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

/** HEIC/HEIF is decoded locally; no conversion changes the original image. */
export function IPhonePicture({ root, path, name }: { root: string; path: string; name: string }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    let generated = "";
    setUrl("");
    setError("");
    const decode = async () => {
      try {
        const encoded = await invoke<string>("read_iphone_media", { root, path });
        if (!active) return;
        const { heicTo } = await import("heic-to/csp");
        const input = new Blob([decodeBase64(encoded)], { type: "image/heic" });
        const result = await heicTo({ blob: input, type: "image/jpeg", quality: 0.8 });
        if (!active) return;
        if (!(result instanceof Blob)) throw Error("Unable to decode HEIC image");
        generated = URL.createObjectURL(result);
        setUrl(generated);
      } catch (cause) {
        if (active) setError(String(cause));
      }
    };
    void decode();
    return () => {
      active = false;
      if (generated) URL.revokeObjectURL(generated);
    };
  }, [root, path]);
  return url ? <img className="photo-image" src={url} alt={name} loading="lazy" /> :
    <div className="photo-fallback"><span>{error ? "HEIC preview unavailable" : "Decoding iPhone photo…"}</span></div>;
}

/** Inline motion playback is attempted locally; codec support varies by Windows install. */
export function IPhoneMotion({ root, path, onOpenExternal }: {
  root: string;
  path: string;
  onOpenExternal: () => void;
}) {
  const [src,setSrc]=useState("");
  const [error,setError]=useState("");
  const [loading,setLoading]=useState(false);
  const [requested,setRequested]=useState(false);
  useEffect(()=>{setRequested(false)},[root,path]);
  useEffect(()=>{
    let active=true;
    setSrc("");setError("");setLoading(false);
    if(!requested)return()=>{active=false};
    setLoading(true);
    void invoke<string>("video_source",{root,path})
      .then(source=>{if(active)setSrc(convertFileSrc(source))})
      .catch(cause=>{if(active)setError(String(cause))})
      .finally(()=>{if(active)setLoading(false)});
    return()=>{active=false};
  },[root,path,requested]);
  return <div className="iphone-motion">
    {src ? <video controls autoPlay playsInline onError={() => setError("Windows WebView2 cannot decode this MOV/HEVC clip. Use the Windows player.")} src={src} style={{maxWidth:"100%",maxHeight:420}} /> :
      <button className="secondary-button" onClick={() => setRequested(true)} disabled={loading}>{loading ? "Loading motion…" : "Play Live Photo / video"}</button>}
    {error && <p>{error}</p>}
    {(error || src) && <button className="secondary-button" onClick={onOpenExternal}>Open in Windows player</button>}
  </div>;
}
