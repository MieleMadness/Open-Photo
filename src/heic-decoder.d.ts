declare module "heic-preview-decoder" {
  interface HeifImage {is_primary():boolean;get_width():number;get_height():number;display(data:ImageData,done:(data:ImageData|null)=>void):void;free():void}
  class HeifDecoder {decoder:number;decode(bytes:Uint8Array):HeifImage[]}
  export default function buildLibheif():{HeifDecoder:typeof HeifDecoder;heif_context_free(context:number):void};
}
