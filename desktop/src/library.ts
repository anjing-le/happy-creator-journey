import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';

export type AssetKind = 'prompt' | 'skill';
export type Modality = 'text' | 'image' | 'video';

export const MODALITY_LABELS: Record<Modality, string> = {
  text: '文',
  image: '图',
  video: '视频',
};

export interface AssetEntry {
  schemaVersion: number;
  id: string;
  kind: AssetKind;
  modality: Modality;
  name: string;
  prompt: string;
  skillMarkdown: string;
  skillDigest?: string | null;
  sample: string;
  tags: string[];
  notes: string;
  images: string[];
  revision: number;
  updatedAt: number;
  source?: string | null;
}

export interface AssetLibrary {
  entries: AssetEntry[];
  imageDirectory: string;
  tags: string[];
  stateToken: string;
}

export function loadLibrary(): Promise<AssetLibrary> {
  if (!isTauri()) return Promise.resolve({ entries: [], imageDirectory: '', tags: [], stateToken: '' });
  return invoke('library_list');
}

export function loadLibraryToken(): Promise<string> {
  if (!isTauri()) return Promise.resolve('');
  return invoke('library_state_token');
}

export function imageUrl(directory: string, name: string): string {
  return convertFileSrc(`${directory}/${name}`);
}

export function skillFiles(id: string): Promise<string[]> {
  return invoke('library_skill_files', { id });
}

export function openSkillFolder(id: string): Promise<void> {
  return invoke('library_open_skill_folder', { id });
}
