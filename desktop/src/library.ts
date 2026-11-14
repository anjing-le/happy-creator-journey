import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
export type AssetKind = 'prompt' | 'skill';
export interface AssetEntry {
  schemaVersion: 1 | 2; id: string; kind: AssetKind; name: string;
  prompt: string; skillMarkdown: string; images: string[]; revision: number; updatedAt: number;
  source?: string | null;
  category: string; tags: string[]; notes: string;
  originCreatedAt?: string | null; originUpdatedAt?: string | null;
  skillDigest?: string | null;
}
export interface AssetLibrary { entries: AssetEntry[]; imageDirectory: string; categories: string[] }
export function emptyEntry(kind: AssetKind): AssetEntry {
  return { schemaVersion: 2, id: '', kind, name: '', prompt: '', skillMarkdown: '', skillDigest: null, images: [], revision: 0, updatedAt: 0, category: '默认', tags: [], notes: '' };
}
export function loadLibrary(): Promise<AssetLibrary> {
  if (!isTauri()) return Promise.resolve({ entries: [], imageDirectory: '', categories: ['默认'] });
  return invoke('library_list');
}
export function saveEntry(entry: AssetEntry, files?: SkillFile[]): Promise<AssetEntry> {
  return invoke('library_save', { entry, expectedRevision: entry.revision, files: files ?? null });
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

export interface SkillFile { path: string; base64: string }
export async function readSkillFolder(files: FileList): Promise<{ name: string; markdown: string; files: SkillFile[] }> {
  const selected = Array.from(files);
  if (!selected.length || selected.length > 512 || selected.reduce((sum, file) => sum + file.size, 0) > 64 * 1024 * 1024) throw new Error('Skill 最多 512 个文件，总计不超过 64 MB');
  const paths = selected.map(file => file.webkitRelativePath.split('/').slice(1).join('/'));
  const root = paths.indexOf('SKILL.md');
  if (root < 0) throw new Error('请选择根目录包含 SKILL.md 的文件夹');
  if (selected[root].size > 256_000) throw new Error('SKILL.md 不超过 256 KB');
  const imported: SkillFile[] = [];
  for (let i = 0; i < selected.length; i++) {
    if (selected[i].size > 16 * 1024 * 1024) throw new Error('Skill 单文件不超过 16 MB');
    const base64 = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(',')[1]); reader.onerror = () => reject(new Error('无法读取 Skill 文件')); reader.readAsDataURL(selected[i]);
    });
    imported.push({ path: paths[i], base64 });
  }
  return { name: selected[0].webkitRelativePath.split('/')[0], markdown: await selected[root].text(), files: imported };
}
export function skillFiles(id: string): Promise<string[]> { return invoke('library_skill_files', { id }); }
export function openSkillFolder(id: string): Promise<void> { return invoke('library_open_skill_folder', { id }); }
