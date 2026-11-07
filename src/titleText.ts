/** Match native Unicode scalar counting and control-character rejection. */
export function validTitleText(text:string){
  return Boolean(text.trim())&&Array.from(text).length<=100&&!/[\p{Cc}\p{Cs}]/u.test(text);
}
