//! Built-in starter vault contents (templates, view schemas, and sample documents).

/// Built-in sample files keyed by Vault-relative path.
pub fn starter_vault_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "_templates/expense-entry.md",
            r#"---
id: {{uuid}}
type: expense
title: "{{title}}"
date: {{date}}
amount: {{amount|0}}
currency: KRW
category: "{{category|식비}}"
payment: "{{payment|체크카드}}"
settled: true
tags:
  - 가계부/{{category|식비}}
---

## 메모
- 
"#,
        ),
        (
            "_templates/trip-plan.md",
            r#"---
id: {{uuid}}
type: trip
title: "{{title}}"
start_date: {{start_date|{{date}}}}
end_date: {{end_date|{{date}}}}
destination: "{{destination|미정}}"
status: 계획중
budget: {{budget|0}}
spent: 0
tags:
  - 여행
---

## 일정 메모
- 1일차: 

## 준비 체크리스트
- [ ] 항공/교통 예약
- [ ] 숙소 예약
- [ ] 통신 준비
"#,
        ),
        (
            ".nodikle/views/expenses.yaml",
            r#"id: expenses
name: "가계부"
source:
  folder: "finance"
  filter:
    op: eq
    property: type
    value: "expense"
properties:
  date:
    label: "날짜"
    type: date
  title:
    label: "항목"
    type: text
  category:
    label: "카테고리"
    type: select
    options: ["식비", "교통", "쇼핑", "주거/통신", "여가"]
  amount:
    label: "금액"
    type: currency
    unit: "KRW"
  payment:
    label: "결제수단"
    type: select
    options: ["체크카드", "신용카드", "현금", "계좌이체"]
  settled:
    label: "정산"
    type: checkbox
views:
  - id: monthly-table
    type: table
    name: "지출 내역"
    group_by: category
    sort:
      - property: date
        dir: desc
    columns: [date, title, category, amount, payment, settled]
    aggregations:
      amount: sum
  - id: expense-calendar
    type: calendar
    name: "지출 캘린더"
    date_property: date
    badge_aggregation:
      property: amount
      op: sum
default_template: "_templates/expense-entry.md"
filename_pattern: "finance/{{date}}-{{slug}}.md"
"#,
        ),
        (
            ".nodikle/views/trips.yaml",
            r#"id: trips
name: "여행 플래너"
source:
  folder: "trips"
  filter:
    op: eq
    property: type
    value: "trip"
properties:
  title:
    label: "여행명"
    type: text
  destination:
    label: "장소"
    type: text
  start_date:
    label: "출발일"
    type: date
  end_date:
    label: "종료일"
    type: date
  status:
    label: "상태"
    type: select
    options: ["아이디어", "계획중", "예약완료", "다녀옴"]
  budget:
    label: "예산"
    type: currency
    unit: "KRW"
  spent:
    label: "실지출"
    type: currency
    unit: "KRW"
views:
  - id: trip-board
    type: board
    name: "상태별 보드"
    group_by: status
    card_fields: [destination, start_date, budget, spent]
  - id: trip-table
    type: table
    name: "전체 일정표"
    sort:
      - property: start_date
        dir: asc
    columns: [title, destination, start_date, end_date, status, budget, spent]
    aggregations:
      budget: sum
      spent: sum
  - id: trip-calendar
    type: calendar
    name: "여행 캘린더"
    date_property: start_date
    end_date_property: end_date
default_template: "_templates/trip-plan.md"
filename_pattern: "trips/{{start_date}}-{{slug}}.md"
"#,
        ),
        (
            "finance/2026-11-27-lunch.md",
            r#"---
id: 01936d4a-7c10-7000-8000-000000000101
type: expense
title: "점심 백반"
date: 2026-11-27
amount: 11000
currency: KRW
category: 식비
payment: 체크카드
settled: true
tags:
  - 가계부/식비
---

동료들과 근처 식당에서 점심 식사.
관련 노트: [[local-first-design]]
"#,
        ),
        (
            "finance/2026-11-26-subway.md",
            r#"---
id: 01936d4a-7c10-7000-8000-000000000102
type: expense
title: "지하철 정기권 충전"
date: 2026-11-26
amount: 55000
currency: KRW
category: 교통
payment: 신용카드
settled: true
tags:
  - 가계부/교통
---

월간 출퇴근 교통비 충전.
"#,
        ),
        (
            "finance/2026-11-25-coffee.md",
            r#"---
id: 01936d4a-7c10-7000-8000-000000000103
type: expense
title: "아메리카노 및 베이커리"
date: 2026-11-25
amount: 8700
currency: KRW
category: 식비
payment: 체크카드
settled: false
tags:
  - 가계부/식비
---

오후 미팅 전 간식 구입.
"#,
        ),
        (
            "trips/2026-12-kyoto.md",
            r#"---
id: 01936d4a-9f20-7000-8000-000000000201
type: trip
title: "2026 겨울 교토 3박 4일"
start_date: 2026-12-18
end_date: 2026-12-21
destination: "일본 교토"
status: 계획중
budget: 1200000
spent: 450000
tags:
  - 여행/일본
---

## 일정 메모
- 1일차: 간사이 공항 -> 교토역 -> 기온 거리 ([[kyoto-food-list]])
- 2일차: 아라시야마 -> 금각사

## 준비 체크리스트
- [x] 왕복 항공권 예매 완료
- [x] 숙소 체크인 시간 확인
- [ ] 교통카드 충전
- [ ] eSIM 개통
"#,
        ),
        (
            "trips/2026-11-busan.md",
            r#"---
id: 01936d4a-9f20-7000-8000-000000000202
type: trip
title: "부산 주말 기차 여행"
start_date: 2026-11-14
end_date: 2026-11-16
destination: "대한민국 부산"
status: 다녀옴
budget: 400000
spent: 365000
tags:
  - 여행/국내
---

## 일정 요약
- KTX 왕복 및 해운대 산책 완료.

## 준비 체크리스트
- [x] KTX 예매
- [x] 숙소 체크아웃
"#,
        ),
        (
            "notes/local-first-design.md",
            r#"---
id: 01936d4a-aa00-7000-8000-000000000301
type: note
title: "로컬 우선 문서 설계 원칙"
tags:
  - 아키텍처/설계
---

# 로컬 우선 문서 설계 원칙

모든 노트는 순수 Markdown 파일로 저장되며 [[2026-12-kyoto|교토 여행 계획]]처럼 자유롭게 연결됩니다.

- [x] Markdown + YAML Frontmatter 파서 구현
- [ ] 클라우드 동기화 엔진 연결
"#,
        ),
        (
            "notes/kyoto-food-list.md",
            r#"---
id: 01936d4a-aa00-7000-8000-000000000302
type: note
title: "교토 맛집 후보"
tags:
  - 여행/일본
---

# 교토 맛집 후보

[[2026-12-kyoto]] 일정에서 방문할 식당 목록입니다.
"#,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use nodikle_model::ViewSchema;

    use super::*;

    #[test]
    fn built_in_view_schemas_deserialize_cleanly() {
        for (path, content) in starter_vault_files() {
            if path.ends_with(".yaml") {
                let schema: ViewSchema =
                    serde_yaml::from_str(content).expect("sample view YAML must be valid");
                assert!(!schema.views.is_empty(), "{path} should define layouts");
            }
        }
    }
}
