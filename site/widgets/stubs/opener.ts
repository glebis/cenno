// Site build: the panel's link opener becomes a plain new-tab open.
export async function openUrl(url: string): Promise<void> {
  window.open(url, "_blank", "noopener");
}
