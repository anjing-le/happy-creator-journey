import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';

export type AssetKind = 'prompt' | 'skill';
export type DeliverableFormat = 'general' | 'cards' | 'article' | 'video';
export type Maturity = 'showcase' | 'draft';

export interface Track {
  name: string;
  positioning: string;
}

export interface IpFoundation {
  name: string;
  visualPrompt: string;
  images: string[];
  voicePrompt: string;
  voiceSample: string;
  tracks: Track[];
  revision: number;
  updatedAt: number;
}

export interface AssetEntry {
  schemaVersion: 1 | 2 | 3;
  id: string;
  kind: AssetKind;
  name: string;
  prompt: string;
  skillMarkdown: string;
  skillDigest?: string | null;
  category: string;
  deliverable: DeliverableFormat;
  track: string;
  maturity: Maturity;
  sampleInput: string;
  sampleOutput: string;
  tags: string[];
  notes: string;
  images: string[];
  revision: number;
  updatedAt: number;
  source?: string | null;
  originCreatedAt?: string | null;
  originUpdatedAt?: string | null;
}

export interface AssetLibrary {
  ip: IpFoundation;
  entries: AssetEntry[];
  imageDirectory: string;
  categories: string[];
  tracks: string[];
}

export const DELIVERABLE_LABELS: Record<DeliverableFormat, string> = {
  general: '通用',
  cards: '卡片图文',
  article: '长文配图',
  video: '视频创作',
};

export function emptyIp(): IpFoundation {
  return { name: '', visualPrompt: '', images: [], voicePrompt: '', voiceSample: '', tracks: [], revision: 0, updatedAt: 0 };
}

export function emptyEntry(kind: AssetKind, deliverable: DeliverableFormat = kind === 'skill' ? 'cards' : 'general'): AssetEntry {
  return {
    schemaVersion: 3,
    id: '',
    kind,
    name: '',
    prompt: '',
    skillMarkdown: '',
    skillDigest: null,
    category: kind === 'skill' ? DELIVERABLE_LABELS[deliverable === 'general' ? 'cards' : deliverable] : '视觉画风',
    deliverable: kind === 'skill' && deliverable === 'general' ? 'cards' : deliverable,
    track: '',
    maturity: 'showcase',
    sampleInput: '',
    sampleOutput: '',
    tags: [],
    notes: '',
    images: [],
    revision: 0,
    updatedAt: 0,
  };
}

export function loadLibrary(): Promise<AssetLibrary> {
  if (!isTauri()) return Promise.resolve({ ip: emptyIp(), entries: [], imageDirectory: '', categories: [], tracks: [] });
  return invoke('library_list');
}

export function saveIp(ip: IpFoundation): Promise<IpFoundation> {
  return invoke('library_save_ip', { ip, expectedRevision: ip.revision });
}

export function saveEntry(entry: AssetEntry, files?: SkillFile[]): Promise<AssetEntry> {
  return invoke('library_save', { entry, expectedRevision: entry.revision, files: files ?? null });
}

export function deleteEntry(entry: AssetEntry): Promise<void> {
  return invoke('library_delete', { id: entry.id, expectedRevision: entry.revision });
}

export async function importImage(file: File): Promise<string> {
  if (file.size > 12 * 1024 * 1024) throw new Error('单张图片请控制在 12 MB 以内');
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error('无法读取图片'));
    reader.readAsDataURL(file);
  });
  return invoke('library_import_image', { dataUrl });
}

export function imageUrl(directory: string, name: string): string {
  return convertFileSrc(`${directory}/${name}`);
}

export interface SkillFile { path: string; base64: string }

export async function readSkillFolder(files: FileList): Promise<{ name: string; markdown: string; files: SkillFile[] }> {
  const selected = Array.from(files);
  if (!selected.length || selected.length > 512 || selected.reduce((sum, file) => sum + file.size, 0) > 64 * 1024 * 1024) {
    throw new Error('Skill 最多 512 个文件，总计不超过 64 MB');
  }
  const paths = selected.map(file => file.webkitRelativePath.split('/').slice(1).join('/'));
  const root = paths.indexOf('SKILL.md');
  if (root < 0) throw new Error('请选择根目录包含 SKILL.md 的文件夹');
  if (selected[root].size > 256_000) throw new Error('SKILL.md 不超过 256 KB');
  const imported: SkillFile[] = [];
  for (let i = 0; i < selected.length; i++) {
    if (selected[i].size > 16 * 1024 * 1024) throw new Error('Skill 单文件不超过 16 MB');
    const base64 = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(',')[1]);
      reader.onerror = () => reject(new Error('无法读取 Skill 文件'));
      reader.readAsDataURL(selected[i]);
    });
    imported.push({ path: paths[i], base64 });
  }
  return { name: selected[0].webkitRelativePath.split('/')[0], markdown: await selected[root].text(), files: imported };
}

export function skillFiles(id: string): Promise<string[]> { return invoke('library_skill_files', { id }); }
export function openSkillFolder(id: string): Promise<void> { return invoke('library_open_skill_folder', { id }); }
export function addCategory(name: string): Promise<string[]> { return invoke('library_add_category', { name }); }
export function moveCategory(entry: AssetEntry, category: string): Promise<AssetEntry> {
  return invoke('library_move_category', { id: entry.id, revision: entry.revision, category });
}
