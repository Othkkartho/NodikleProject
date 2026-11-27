import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type {
  DocumentDetail,
  DocumentMeta,
  FilterExpr,
  PropertyPatch,
  PropertyValue,
  ReindexReport,
  SaveResult,
  SearchHit,
  TagCount,
  TemplateInfo,
  VaultStats,
  ViewQueryResult,
  ViewSchema,
} from '../types/nodikle';

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return previewFallback<T>(cmd, args);
  }
  return invoke<T>(cmd, args);
}

export const nodikleApi = {
  isDesktopOrMobileNative: isTauriRuntime,

  vaultGetDefaultPath: () => call<string>('vault_get_default_path'),
  vaultInitSample: (vaultPath: string, overwrite = false) =>
    call<VaultStats>('vault_init_sample', { vaultPath, overwrite }),
  vaultOpen: (vaultPath: string) => call<VaultStats>('vault_open', { vaultPath }),
  vaultReindex: (fullRebuild = false) => call<ReindexReport>('vault_reindex', { fullRebuild }),
  vaultStats: () => call<VaultStats>('vault_stats'),

  docList: (folder?: string, tagPrefix?: string) =>
    call<DocumentMeta[]>('doc_list', { folder, tagPrefix }),
  docGet: (relPath: string) => call<DocumentDetail>('doc_get', { relPath }),
  docSave: (relPath: string, rawMd: string, expectedHash?: string) =>
    call<SaveResult>('doc_save', { relPath, rawMd, expectedHash }),
  docDelete: (relPath: string) => call<void>('doc_delete', { relPath }),
  docPatchProperties: (relPath: string, patches: PropertyPatch[]) =>
    call<SaveResult>('doc_patch_properties', { relPath, patches }),
  docToggleTask: (relPath: string, lineNo: number, completed: boolean) =>
    call<SaveResult>('doc_toggle_task', { relPath, lineNo, completed }),

  templateList: () => call<TemplateInfo[]>('template_list'),
  docCreateFromTemplate: (
    title: string,
    options: {
      viewId?: string;
      templatePath?: string;
      vars?: Record<string, PropertyValue>;
    }
  ) =>
    call<DocumentDetail>('doc_create_from_template', {
      viewId: options.viewId,
      templatePath: options.templatePath,
      title,
      vars: options.vars,
    }),

  viewList: () => call<ViewSchema[]>('view_list'),
  viewQuery: (
    viewId: string,
    layoutId?: string,
    runtimeFilter?: FilterExpr,
    page = 0,
    limit = 200
  ) => call<ViewQueryResult>('view_query', { viewId, layoutId, runtimeFilter, page, limit }),

  searchFts: (query: string, tagPrefix?: string, folder?: string, limit = 50) =>
    call<SearchHit[]>('search_fts', { query, tagPrefix, folder, limit }),
  tagsList: () => call<TagCount[]>('tags_list'),
  wikilinkResolve: (rawTarget: string) => call<string | null>('wikilink_resolve', { rawTarget }),

  onVaultFileChanged: async (
    handler: (relPaths: string[]) => void
  ): Promise<UnlistenFn> => {
    if (!isTauriRuntime()) {
      return () => {};
    }
    return listen<{ rel_paths: string[] }>('vault:file_changed', (evt) => {
      handler(evt.payload.rel_paths ?? []);
    });
  },
};

// Browser preview fallback when Vite dev server is opened directly outside Tauri.
const previewDocs: DocumentDetail[] = [
  {
    meta: {
      id: 'preview-1',
      rel_path: 'notes/welcome.md',
      stem: 'welcome',
      folder: 'notes',
      title: 'Nodikle 브라우저 미리보기',
      doc_type: 'note',
      content_hash: 'preview',
      mtime_ms: Date.now(),
      size_bytes: 120,
      created_at: '2026-11-27T00:00:00Z',
      updated_at: '2026-11-27T00:00:00Z',
    },
    raw_md: '---\ntitle: Nodikle 브라우저 미리보기\n---\n\nTauri 데스크톱/모바일 앱에서 실행하면 로컬 Vault와 SQLite 인덱스가 활성화됩니다.\n',
    body_md: '\nTauri 데스크톱/모바일 앱에서 실행하면 로컬 Vault와 SQLite 인덱스가 활성화됩니다.\n',
    properties: { title: { kind: 'text', value: 'Nodikle 브라우저 미리보기' } },
    ordered_keys: ['title'],
    tags: ['미리보기'],
    wikilinks: [],
    backlinks: [],
    tasks: [],
  },
];

function previewFallback<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case 'vault_get_default_path':
      return Promise.resolve('~/NodikleVault' as T);
    case 'vault_init_sample':
    case 'vault_open':
    case 'vault_stats':
      return Promise.resolve({
        vault_path: '~/NodikleVault',
        doc_count: previewDocs.length,
        tag_count: 1,
        link_count: 0,
        task_count: 0,
      } as T);
    case 'doc_list':
      return Promise.resolve(previewDocs.map((d) => d.meta) as T);
    case 'doc_get':
      return Promise.resolve(
        (previewDocs.find((d) => d.meta.rel_path === args?.relPath) ?? previewDocs[0]) as T
      );
    case 'view_list':
    case 'template_list':
    case 'search_fts':
      return Promise.resolve([] as unknown as T);
    case 'tags_list':
      return Promise.resolve([{ tag: '미리보기', count: 1 }] as T);
    default:
      return Promise.resolve(null as T);
  }
}
