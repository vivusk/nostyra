/** Whether a desktop Data folder path lies inside the Windows AppData folder (the old
 *  default, `%APPDATA%\Nostyra\data`). Chrome refuses to open folders there, so the
 *  web edition cannot share such a folder. Import-free for `node --test`. */
export function isInsideAppData(path: string): boolean {
  return /[\\/]AppData[\\/]/i.test(path);
}
