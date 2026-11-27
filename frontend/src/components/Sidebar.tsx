import { useState } from 'react';
import {
  Database,
  FileText,
  FolderOpen,
  Hash,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  X,
} from 'lucide-react';

import { useWorkspaceStore } from '../store/useWorkspaceStore';

export function Sidebar() {
  const {
    vaultPath,
    stats,
    views,
    documents,
    tags,
    selectedTag,
    searchQuery,
    searchHits,
    mainMode,
    activeViewId,
    activeDoc,
    openOrInitVault,
    selectTag,
    runSearch,
    openView,
    openDocument,
    createBlankNote,
    setMobileSidebarOpen,
  } = useWorkspaceStore();

  const [pathInput, setPathInput] = useState(vaultPath);
  const [newTitle, setNewTitle] = useState('');
  const [showNewInput, setShowNewInput] = useState(false);

  const handleOpenPath = (initSample: boolean) => {
    const target = (pathInput || vaultPath).trim();
    if (target) {
      void openOrInitVault(target, initSample);
    }
  };

  const handleCreateNote = () => {
    if (!newTitle.trim()) return;
    void createBlankNote(newTitle.trim());
    setNewTitle('');
    setShowNewInput(false);
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="brand-row">
          <div className="brand-mark">N</div>
          <div>
            <div className="brand-title">Nodikle</div>
            <div className="brand-sub">
              {stats ? `${stats.doc_count}문서 · ${stats.tag_count}태그` : '로컬 워크스페이스'}
            </div>
          </div>
          <button
            className="icon-btn mobile-only"
            aria-label="사이드바 닫기"
            onClick={() => setMobileSidebarOpen(false)}
          >
            <X size={16} />
          </button>
        </div>

        <div className="vault-bar">
          <input
            className="vault-input"
            value={pathInput || vaultPath}
            onChange={(e) => setPathInput(e.target.value)}
            placeholder="Vault 경로 입력..."
            aria-label="Vault 경로"
          />
          <button
            className="icon-btn"
            title="Vault 열기"
            onClick={() => handleOpenPath(false)}
          >
            <FolderOpen size={15} />
          </button>
          <button
            className="icon-btn"
            title="샘플 Vault 채우기"
            onClick={() => handleOpenPath(true)}
          >
            <Sparkles size={15} />
          </button>
          <button
            className="icon-btn"
            title="인덱스 새로고침"
            onClick={() => void useWorkspaceStore.getState().refreshWorkspaceData()}
          >
            <RefreshCw size={15} />
          </button>
        </div>

        <div className="search-box">
          <Search size={14} className="search-icon" />
          <input
            value={searchQuery}
            onChange={(e) => void runSearch(e.target.value)}
            placeholder="전문 검색 (제목, 본문)..."
            aria-label="전문 검색"
          />
          {searchQuery && (
            <button
              className="icon-btn"
              aria-label="검색 지우기"
              onClick={() => void runSearch('')}
            >
              <X size={13} />
            </button>
          )}
        </div>
      </div>

      <div className="sidebar-scroll">
        {searchQuery.trim() ? (
          <section className="sidebar-section">
            <div className="section-title">검색 결과 ({searchHits.length})</div>
            {searchHits.length === 0 ? (
              <div className="empty-hint">일치하는 문서가 없습니다.</div>
            ) : (
              <ul className="nav-list">
                {searchHits.map((hit) => (
                  <li key={hit.doc_id}>
                    <button
                      className={`nav-item ${
                        mainMode === 'editor' && activeDoc?.meta.rel_path === hit.rel_path
                          ? 'active'
                          : ''
                      }`}
                      onClick={() => void openDocument(hit.rel_path)}
                    >
                      <FileText size={14} />
                      <div className="search-hit-body">
                        <div className="nav-label">{hit.title}</div>
                        {hit.snippet && (
                          <div className="search-snippet">
                            {hit.snippet.replace(/<\/?mark>/g, '')}
                          </div>
                        )}
                      </div>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        ) : (
          <>
            <section className="sidebar-section">
              <div className="section-title">데이터베이스 뷰</div>
              <ul className="nav-list">
                {views.map((view) => (
                  <li key={view.id}>
                    <button
                      className={`nav-item ${
                        mainMode === 'view' && activeViewId === view.id ? 'active' : ''
                      }`}
                      onClick={() => void openView(view.id)}
                    >
                      <Database size={14} />
                      <span className="nav-label">{view.name}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </section>

            <section className="sidebar-section">
              <div className="section-header">
                <span className="section-title">
                  문서 ({documents.length})
                  {selectedTag ? ` · #${selectedTag}` : ''}
                </span>
                <button
                  className="icon-btn"
                  title="새 노트 만들기"
                  onClick={() => setShowNewInput((prev) => !prev)}
                >
                  <Plus size={14} />
                </button>
              </div>

              {showNewInput && (
                <div className="inline-create-row">
                  <input
                    value={newTitle}
                    onChange={(e) => setNewTitle(e.target.value)}
                    placeholder="새 노트 제목..."
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleCreateNote();
                    }}
                  />
                  <button className="btn-primary-sm" onClick={handleCreateNote}>
                    생성
                  </button>
                </div>
              )}

              <ul className="nav-list">
                {documents.map((doc) => (
                  <li key={doc.id}>
                    <button
                      className={`nav-item ${
                        mainMode === 'editor' && activeDoc?.meta.rel_path === doc.rel_path
                          ? 'active'
                          : ''
                      }`}
                      onClick={() => void openDocument(doc.rel_path)}
                    >
                      <FileText size={14} />
                      <span className="nav-label">{doc.title}</span>
                      <span className="nav-badge">{doc.folder || '/'}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </section>

            <section className="sidebar-section">
              <div className="section-header">
                <span className="section-title">태그</span>
                {selectedTag && (
                  <button className="text-btn" onClick={() => void selectTag(null)}>
                    필터 해제
                  </button>
                )}
              </div>
              <div className="tag-cloud">
                {tags.map((item) => (
                  <button
                    key={item.tag}
                    className={`tag-chip ${selectedTag === item.tag ? 'active' : ''}`}
                    onClick={() =>
                      void selectTag(selectedTag === item.tag ? null : item.tag)
                    }
                  >
                    <Hash size={11} />
                    <span>{item.tag}</span>
                    <span className="tag-count">{item.count}</span>
                  </button>
                ))}
              </div>
            </section>
          </>
        )}
      </div>
    </aside>
  );
}
