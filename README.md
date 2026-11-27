# Nodikle

**Nodikle**는 로컬 우선(Local-first) 기반의 가벼운 Markdown 지식 관리와 구조화된 속성 기반 데이터베이스 뷰, 그리고 개인 클라우드 스토리지 동기화를 하나로 결합한 크로스플랫폼 문서 관리 프로그램입니다.

---

## 지원 플랫폼 및 현황

- **1차 목표 (MVP):** Desktop (`Windows` / `macOS` / `Linux`) + **Android**
- **추후 확장:** **iOS**
- **현재 구현 현황 (`v0.1.0`):** Phase 1(Rust 코어 · SQLite FTS5 인덱스) 및 Phase 2(Tauri v2 앱 호스트 · 하이브리드 에디터 · 테이블/칸반/캘린더 뷰 · 가계부/여행 플래너 · 모바일 카드/퀵 입력 UX) 구현 완료. (클라우드 동기화는 Phase 3 진행 예정)

---

## 주요 기능

- **로컬 우선 Markdown 노트 & 지식 연결:** 순수 `.md` + YAML Frontmatter 저장, `[[문서명]]` 양방향 링크(Backlink), 중첩 태그(`#분류/세부`), SQLite FTS5 고속 전문 검색
- **구조화된 문서 & 데이터베이스 뷰:** Frontmatter 속성 기반 **테이블**, **칸반 보드**, **캘린더** 뷰 및 실시간 필터·정렬·합계 집계, **가계부**·**여행 플래너** 템플릿 시스템
- **모바일 최적화 UX:** 좁은 화면 자동 카드 뷰 전환, 상태 세그먼트 탭, 하단 퀵 입력 바텀 시트 및 고정 요약 바

---

## 빠른 실행 (소스 빌드)

```bash
# 1. 앱 아이콘 생성 및 프론트엔드 빌드
node scripts/generate-icons.mjs
cd frontend && npm install && npm run build && cd ..

# 2. 데스크톱 앱 실행
cargo run -p nodikle-app

# 3. 전체 테스트 실행
cargo test --workspace
```

---

## 상세 설계 문서

- [01. 전체 시스템 아키텍처 및 크로스플랫폼 UI 전략](docs/01-architecture.md)
- [02. 하이브리드 문서·데이터 모델 및 데이터베이스 뷰 설계](docs/02-data-model-and-views.md)
- [03. 확장 가능한 저장소 및 클라우드 동기화 아키텍처](docs/03-storage-and-sync.md)
- [04. 단계별 구현 로드맵 및 검증 기준](docs/04-roadmap.md)

---

## 기여 및 라이선스

- 기여 가이드: [CONTRIBUTING.md](CONTRIBUTING.md) · 행동 강령: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- 라이선스: [MIT License](LICENSE)
