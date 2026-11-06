// Single files the user picks (Settings → BIOS → "Add BIOS file…"): the file
// picker hands out an opaque ref (`web-file:<uuid>`, the File kept in memory) that
// the command receives where the desktop receives a path.

const PICK_REF = "web-file:";
const picked = new Map<string, File>();

/** Shows the file picker; the ref for the command, or null when cancelled. */
export async function pickFile(): Promise<string | null> {
  if (!window.showOpenFilePicker) throw new Error("This browser cannot open files. Use Chrome or Edge.");
  try {
    const [handle] = await window.showOpenFilePicker({ multiple: false });
    const ref = `${PICK_REF}${crypto.randomUUID()}`;
    picked.set(ref, await handle.getFile());
    return ref;
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") return null;
    throw error;
  }
}

/** The picked File of a ref (once: the ref is forgotten). */
export function takePickedFile(ref: string): File | null {
  const file = picked.get(ref) ?? null;
  picked.delete(ref);
  return file;
}
