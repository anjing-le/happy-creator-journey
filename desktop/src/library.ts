import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
export type AssetKind = 'ip' | 'scene';
export interface AssetEntry {
  schemaVersion: 1; id: string; kind: AssetKind; name: string;
  prompt: string; skillMarkdown: string; images: string[]; revision: number; updatedAt: number;
  source?: string | null;
}
export interface AssetLibrary { entries: AssetEntry[]; imageDirectory: string }
export function emptyEntry(kind: AssetKind): AssetEntry {
  return { schemaVersion: 1, id: '', kind, name: '', prompt: '', skillMarkdown: '', images: [], revision: 0, updatedAt: 0 };
}
export function loadLibrary(): Promise<AssetLibrary> {
  if (!isTauri()) return Promise.resolve({ entries: [], imageDirectory: '' });
  return invoke('library_list');
}
export function saveEntry(entry: AssetEntry): Promise<AssetEntry> {
  return invoke('library_save', { entry, expectedRevision: entry.revision });
}
export async function importImage(file: File): Promise<string> {
  if (file.size > 12 * 1024 * 1024) throw new Error('单张图片请控制在 12 MB 以内');
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader(); reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error('无法读取图片')); reader.readAsDataURL(file);
  });
  return invoke('library_import_image', { dataUrl });
}
export function imageUrl(directory: string, name: string): string {
  return convertFileSrc(`${directory}/${name}`);
}
