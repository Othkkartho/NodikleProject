import { create } from 'zustand';

import { nodikleApi } from '../api/bridge';
import type {
  DocumentDetail,
  DocumentMeta,
  FilterExpr,
  PropertyPatch,
  PropertyValue,
  SearchHit,
  TagCount,
  TemplateInfo,
  VaultStats,
  ViewQueryResult,
  ViewSchema,
} from '../types/nodikle';

export type MainMode = 'editor' | 'view';

interface WorkspaceState {
  vaultPath: string;
  stats: VaultStats | null;
  views: ViewSchema[];
  templates: TemplateInfo[];
  documents: DocumentMeta[];
  tags: TagCount[];
  selectedTag: string | null;
  searchQuery: string;
  searchHits: SearchHit[];

  mainMode: MainMode;
  activeDoc: DocumentDetail | null;
  activeViewId: string | null;
  activeLayoutId: string | null;
  viewResult: ViewQueryResult | null;
  runtimeFilter: FilterExpr | null;

  loading: boolean;
  errorMessage: string | null;
  mobileSidebarOpen: boolean;
  quickCaptureOpen: boolean;
  isMobileViewport: boolean;

  setMobileViewport: (isMobile: boolean) => void;
  setMobileSidebarOpen: (open: boolean) => void;
  setQuickCaptureOpen: (open: boolean) => void;
  clearError: () => void;

  bootstrap: () => Promise<void>;
  openOrInitVault: (path: string, initSample: boolean) => Promise<void>;
  refreshWorkspaceData: () => Promise<void>;
  selectTag: (tag: string | null) => Promise<void>;
  runSearch: (query: string) => Promise<void>;

  openDocument: (relPath: string) => Promise<void>;
  saveActiveDocument: (rawMd: string) => Promise<void>;
  patchDocumentProperties: (relPath: string, patches: PropertyPatch[]) => Promise<void>;
  toggleTask: (relPath: string, lineNo: number, completed: boolean) => Promise<void>;
  createBlankNote: (title: string) => Promise<void>;
  createFromTemplate: (
    title: string,
    vars: Record<string, PropertyValue>,
    viewId?: string
  ) => Promise<void>;
  deleteDocument: (relPath: string) => Promise<void>;
  openWikilink: (rawTarget: string) => Promise<void>;

  openView: (viewId: string, layoutId?: string) => Promise<void>;
  setRuntimeFilter: (filter: FilterExpr | null) => Promise<void>;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  vaultPath: '',
  stats: null,
  views: [],
  templates: [],
  documents: [],
  tags: [],
  selectedTag: null,
  searchQuery: '',
  searchHits: [],

  mainMode: 'view',
  activeDoc: null,
  activeViewId: null,
  activeLayoutId: null,
  viewResult: null,
  runtimeFilter: null,

  loading: false,
  errorMessage: null,
  mobileSidebarOpen: false,
  quickCaptureOpen: false,
  isMobileViewport: typeof window !== 'undefined' ? window.innerWidth < 640 : false,

  setMobileViewport: (isMobile) => set({ isMobileViewport: isMobile }),
  setMobileSidebarOpen: (open) => set({ mobileSidebarOpen: open }),
  setQuickCaptureOpen: (open) => set({ quickCaptureOpen: open }),
  clearError: () => set({ errorMessage: null }),

  bootstrap: async () => {
    set({ loading: true, errorMessage: null });
    try {
      const defaultPath = await nodikleApi.vaultGetDefaultPath();
      const stats = await nodikleApi.vaultInitSample(defaultPath, false);
      set({ vaultPath: stats.vault_path, stats });
      await get().refreshWorkspaceData();

      const { views, documents } = get();
      if (views.length > 0) {
        await get().openView(views[0].id);
      } else if (documents.length > 0) {
        await get().openDocument(documents[0].rel_path);
      }
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    } finally {
      set({ loading: false });
    }
  },

  openOrInitVault: async (path, initSample) => {
    set({ loading: true, errorMessage: null });
    try {
      const stats = initSample
        ? await nodikleApi.vaultInitSample(path, false)
        : await nodikleApi.vaultOpen(path);
      set({ vaultPath: stats.vault_path, stats, activeDoc: null, viewResult: null });
      await get().refreshWorkspaceData();
      const { views, documents } = get();
      if (views.length > 0) {
        await get().openView(views[0].id);
      } else if (documents.length > 0) {
        await get().openDocument(documents[0].rel_path);
      }
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    } finally {
      set({ loading: false });
    }
  },

  refreshWorkspaceData: async () => {
    const tag = get().selectedTag ?? undefined;
    const [stats, views, templates, documents, tags] = await Promise.all([
      nodikleApi.vaultStats(),
      nodikleApi.viewList(),
      nodikleApi.templateList(),
      nodikleApi.docList(undefined, tag),
      nodikleApi.tagsList(),
    ]);
    set({ stats, views, templates, documents, tags });
  },

  selectTag: async (tag) => {
    set({ selectedTag: tag });
    const documents = await nodikleApi.docList(undefined, tag ?? undefined);
    set({ documents });
  },

  runSearch: async (query) => {
    set({ searchQuery: query });
    if (!query.trim()) {
      set({ searchHits: [] });
      return;
    }
    try {
      const hits = await nodikleApi.searchFts(query, get().selectedTag ?? undefined);
      set({ searchHits: hits });
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  openDocument: async (relPath) => {
    try {
      const doc = await nodikleApi.docGet(relPath);
      set({
        activeDoc: doc,
        mainMode: 'editor',
        mobileSidebarOpen: false,
      });
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  saveActiveDocument: async (rawMd) => {
    const active = get().activeDoc;
    if (!active) return;
    try {
      await nodikleApi.docSave(active.meta.rel_path, rawMd, active.meta.content_hash);
      const updated = await nodikleApi.docGet(active.meta.rel_path);
      set({ activeDoc: updated });
      await get().refreshWorkspaceData();
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  patchDocumentProperties: async (relPath, patches) => {
    try {
      await nodikleApi.docPatchProperties(relPath, patches);
      await get().refreshWorkspaceData();
      const { activeDoc, mainMode, activeViewId, activeLayoutId } = get();
      if (activeDoc?.meta.rel_path === relPath) {
        const refreshed = await nodikleApi.docGet(relPath);
        set({ activeDoc: refreshed });
      }
      if (mainMode === 'view' && activeViewId) {
        await get().openView(activeViewId, activeLayoutId ?? undefined);
      }
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  toggleTask: async (relPath, lineNo, completed) => {
    try {
      await nodikleApi.docToggleTask(relPath, lineNo, completed);
      const refreshed = await nodikleApi.docGet(relPath);
      set({ activeDoc: refreshed });
      await get().refreshWorkspaceData();
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  createBlankNote: async (title) => {
    const clean = title.trim() || '새 노트';
    const slug = clean
      .toLowerCase()
      .replace(/[^0-9a-z가-힣]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'note';
    const relPath = `notes/${slug}-${Date.now().toString().slice(-4)}.md`;
    const raw = `---\ntitle: "${clean}"\ntype: note\ntags: []\n---\n\n# ${clean}\n\n`;
    try {
      await nodikleApi.docSave(relPath, raw);
      await get().refreshWorkspaceData();
      await get().openDocument(relPath);
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  createFromTemplate: async (title, vars, viewId) => {
    try {
      const created = await nodikleApi.docCreateFromTemplate(title, {
        viewId,
        vars,
      });
      await get().refreshWorkspaceData();
      if (viewId && get().mainMode === 'view') {
        await get().openView(viewId, get().activeLayoutId ?? undefined);
      } else {
        await get().openDocument(created.meta.rel_path);
      }
      set({ quickCaptureOpen: false });
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  deleteDocument: async (relPath) => {
    try {
      await nodikleApi.docDelete(relPath);
      const { activeDoc, views } = get();
      if (activeDoc?.meta.rel_path === relPath) {
        set({ activeDoc: null });
      }
      await get().refreshWorkspaceData();
      if (views.length > 0 && !get().activeDoc) {
        await get().openView(views[0].id);
      }
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  openWikilink: async (rawTarget) => {
    try {
      const resolved = await nodikleApi.wikilinkResolve(rawTarget);
      if (resolved) {
        await get().openDocument(resolved);
      } else {
        await get().createBlankNote(rawTarget);
      }
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  openView: async (viewId, layoutId) => {
    try {
      const schema = get().views.find((v) => v.id === viewId);
      const targetLayoutId = layoutId ?? schema?.views[0]?.id ?? undefined;
      const result = await nodikleApi.viewQuery(
        viewId,
        targetLayoutId,
        get().runtimeFilter ?? undefined
      );
      set({
        mainMode: 'view',
        activeViewId: viewId,
        activeLayoutId: result.layout_id,
        viewResult: result,
        mobileSidebarOpen: false,
      });
    } catch (err) {
      set({ errorMessage: formatErr(err) });
    }
  },

  setRuntimeFilter: async (filter) => {
    set({ runtimeFilter: filter });
    const { activeViewId, activeLayoutId } = get();
    if (activeViewId) {
      await get().openView(activeViewId, activeLayoutId ?? undefined);
    }
  },
}));

function formatErr(err: unknown): string {
  if (typeof err === 'object' && err !== null && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}
