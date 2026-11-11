// Site build: stands in for Tauri's `invoke`. Only `load_image` is used by the
// views; it fetches a same-origin path and returns a data: URI, the shape the
// app's Rust loader returns. Failures throw a plain reason string, which
// ImageView shows verbatim — the same contract as the app.
export async function invoke<T>(cmd: string, args?: { path?: string }): Promise<T> {
  if (cmd !== "load_image" || !args?.path) throw `unsupported command: ${cmd}`;
  const res = await fetch(args.path);
  if (!res.ok) throw "file not found";
  const blob = await res.blob();
  if (!blob.type.startsWith("image/")) throw "not an image";
  return new Promise<T>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result as T);
    r.onerror = () => reject("failed to decode");
    r.readAsDataURL(blob);
  });
}
