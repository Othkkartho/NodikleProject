import { useEffect } from 'react';
import { Menu, Plus, X } from 'lucide-react';

import { nodikleApi } from './api/bridge';
import { DatabaseView } from './components/DatabaseView';
import { HybridEditor } from './components/HybridEditor';
import { QuickCaptureSheet } from './components/QuickCaptureSheet';
import { Sidebar } from './components/Sidebar';
import { useWorkspaceStore } from './store/useWorkspaceStore';

export function App() {
  const {
    mainMode,
    loading,
    errorMessage,
    mobileSidebarOpen,
    views,
    bootstrap,
    clearError,
    setMobileViewport,
    setMobileSidebarOpen,
    setQuickCaptureOpen,
    refreshWorkspaceData,
    openView,
    openDocument,
  } = useWorkspaceStore();

  useEffect(() => {
    void bootstrap();

    const onResize = () => setMobileViewport(window.innerWidth < 640);
    window.addEventListener('resize', onResize);

    let unlisten: (() => void) | undefined;
    void nodikleApi.onVaultFileChanged((relPaths) => {
      void refreshWorkspaceData();
      const state = useWorkspaceStore.getState();
      if (state.activeDoc && relPaths.includes(state.activeDoc.meta.rel_path)) {
        void openDocument(state.activeDoc.meta.rel_path);
      }
      if (state.mainMode === 'view' && state.activeViewId) {
        void openView(state.activeViewId, state.activeLayoutId ?? undefined);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      window.removeEventListener('resize', onResize);
      unlisten?.();
    };
  }, []);

  return (
    <div className="app-shell">
      <div className={`sidebar-drawer ${mobileSidebarOpen ? 'open' : ''}`}>
        <Sidebar />
      </div>
      {mobileSidebarOpen && (
        <div
          className="drawer-backdrop mobile-only"
          onClick={() => setMobileSidebarOpen(false)}
        />
      )}

      <main className="main-pane">
        <div className="mobile-topbar mobile-only">
          <button
            className="icon-btn"
            aria-label="메뉴 열기"
            onClick={() => setMobileSidebarOpen(true)}
          >
            <Menu size={18} />
          </button>
          <span className="mobile-brand">Nodikle</span>
          <div />
        </div>

        {errorMessage && (
          <div className="error-banner">
            <span>{errorMessage}</span>
            <button className="icon-btn" onClick={clearError}>
              <X size={14} />
            </button>
          </div>
        )}

        {loading ? (
          <div className="empty-stage">Vault 인덱스를 불러오는 중입니다...</div>
        ) : mainMode === 'view' ? (
          <DatabaseView />
        ) : (
          <HybridEditor />
        )}
      </main>

      {views.length > 0 && (
        <button
          className="fab-button"
          aria-label="빠른 항목 입력"
          onClick={() => setQuickCaptureOpen(true)}
        >
          <Plus size={22} />
        </button>
      )}

      <QuickCaptureSheet />
    </div>
  );
}
