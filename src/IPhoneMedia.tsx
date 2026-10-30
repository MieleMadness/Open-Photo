import {VideoPlayer} from "./VideoPlayer";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

/** A new media identity resets all transient player state. */
export function IPhoneMotion(props: {root:string;path:string;onOpenExternal:()=>void;autoLoad?:boolean}) {
  return <VideoPlayer key={`${props.root}\0${props.path}`} {...props}/>;
}
