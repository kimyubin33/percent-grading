# % 채점학 · Percent Grading

**타당한 추론을 인정하면서도, 근거 없는 결론을 드러내는 채점 프로그램입니다.**

[English](README.md) | [한국어](README.ko.md) | [Rust 실행 안내](docs/RUST_GUIDE.md) | [이론 검색·선택 안내](docs/RESEARCH_GUIDE.md) | [영문 교재](book/Percent-Grading-Textbook.md)

제안자: **김유빈(Yubin Kim)**. 현재 구현은 **Rust 0.3.0**이며, 초기 Python 0.1.1 교육용 프로토타입도 보존합니다.

## 왜 만들기 시작했는가

정답과 다른 답안에도 올바른 구성 요소, 타당한 원리, 특정 조건에서 성립하는 결론이 있을 수 있습니다. 정답·오답이라는 하나의 표시로는 이러한 차이가 사라집니다.

출발점은 A라는 답을 요구할 때 A만 인정해야 하느냐는 질문이었습니다. 다른 기호가 A의 일부 구성 요소를 공유한다면 그 부분은 인정할 수 있지만, 그것만으로 전체 기호가 A와 같아지지는 않습니다.

> 이 답안에서 무엇이 맞고, 무엇이 틀렸으며, 어떤 조건에서 성립하고, 무엇을 아직 판단하지 못했는가?

기존에도 부분점수와 분석적 채점 기준은 존재합니다. 이 프로젝트는 명시적인 언어 규칙과 AI를 결합해 다양한 관점을 일관되고 검토 가능한 방법으로 적용하는 방법을 연구합니다.

## Rust 프로그램의 작동 방식

1. 실제 [RelSpec/AHAA 컴파일러](https://github.com/kimyubin33/relspec-lang)로 채점 규칙을 검사합니다.
2. 배점·판정 항목·적용 조건을 검증하고 내용 해시로 규칙을 고정합니다.
3. 필요하면 문제에 관련된 관점을 카탈로그에서 자동 선택하고, 출처와 선택 이유를 기록합니다.
4. **OpenAI와 Claude 두 AI만** 연결해 고정된 판정 항목을 선택하고 답안 근거를 인용하게 합니다.
5. 두 AI가 같은 항목에 동의하면 프로그램이 고정 점수를 계산합니다.
6. 불일치한 항목은 범위와 검토 상태로 남기고, 저장된 자료로 결과를 재생합니다.

AI는 근거를 분류하고, **허용된 점수 범위와 계산은 Rust가 통제합니다.** 두 AI의 의미 해석이 항상 같다고 보장하지 않습니다. 의견이 다르면 평균으로 감추지 않습니다. AHAA의 원래 문법 키워드는 한국어를 유지하며, 식별자·프로그램 출력·기술 문서·교재는 영어입니다.

## 사과 예제

“사과는 무슨 색인가?”만으로는 특정 사과나 관찰 조건이 정해지지 않습니다. 예제는 넓은 백색광 아래의 붉은 껍질 사과를 명시하고, 색과 광학 원리 및 적용 조건을 설명하도록 요구합니다.

| 답안에 나타난 내용 | 평가하는 항목 |
|---|---|
| 지정한 사과가 빨갛다는 결론 | 문제 의도 충족 |
| 반사와 흡수 등 관련 원리 | 원리의 타당성 |
| 그 원리에서 해당 결론을 도출하는 연결 | 추론의 타당성 |
| 관찰 조건이나 바뀐 상황의 구분 | 조건의 명확성 |
| 검증 가능한 미래 변화나 대안 제안 | 별도의 미래 탐색 점수 |

“사과는 파랗다. 빛이 반사되기 때문이다”에는 맞는 일반 원리가 포함되어도, 지정된 조건에서 파란색이라는 결론까지 뒷받침하지는 않습니다. 파란 조명을 비춘다는 사실만으로 붉은 표면이 반드시 파랗게 보이지도 않습니다. 타당한 부분과 실패한 결론을 함께 기록합니다.

단순히 색만 묻는 문제에 추론 요구를 뒤늦게 추가해서는 안 됩니다. 짧은 “빨간색” 답안을 어떻게 평가할지는 답안을 보기 전에 정한 문제와 평가 기준에 따라야 합니다.

## 무엇의 퍼센트인가

현재 점수는 의도·원리·추론·조건에 대해 명시한 기준의 가중 충족도입니다. 미래 탐색 점수는 제안한 가설이 얼마나 구체적이고 검증 가능하게 제시되었는지 나타냅니다. **미래 점수가 현재 사실의 정확도를 올리지는 않습니다.**

이 수치는 답안이 참일 확률이나 미래 성공 확률이 아닙니다. 점수 범위도 두 AI의 공통 오판을 보장하는 통계적 신뢰구간이 아닙니다.

## 이론·관점 자동 검색과 선택

0.3.0은 다음 두 기능을 구분합니다.

- **서지정보 검색**: Crossref에서 관련 출판물의 제목·DOI·발행 정보 등을 찾습니다. 검색 결과를 곧바로 검증된 이론이나 채점 근거로 승격하지 않습니다.
- **카탈로그 자동 선택**: 출처와 가정을 기록한 관점을 고정된 문제에 맞춰 선택합니다. 답안은 선택 입력에 포함하지 않으며, 같은 중복 그룹은 하나만 선택합니다.

선택한 관점, 일치 키워드, 연결된 평가 항목, 제외 이유와 미포함 항목을 기록합니다. 문맥 크기와 선택 수를 제한하고, 카탈로그 및 선택 설정을 스냅샷으로 고정합니다. 카탈로그를 수정하면 별도의 실행으로 구분합니다.

예제 카탈로그에는 관점 8개가 들어 있습니다. 프로그램의 대규모 입력 처리 능력이 수만 가지 이론의 정확성을 검증했다는 뜻은 아닙니다. 키워드 일치는 의미적 관련성을 보장하지 않고, 중복 그룹도 작성자가 정합니다. 추가 관점은 기존 배점과 판정 항목을 바꿀 수 없습니다.

## API 없이 실행

Rust 환경을 설치하고 저장소를 복제한 뒤, 저장소 폴더에서 실행합니다.

~~~text
cargo build --locked
cargo run --locked -- compile policies/apple.aha --out local-runs/apple.lock.json
cargo run --locked -- demo local-runs/apple.lock.json examples/rust/apple-blue.fixture.json
cargo run --locked -- select local-runs/apple.lock.json catalogs/apple.json --out local-runs/apple.research.json
cargo run --locked -- inspect-research local-runs/apple.lock.json local-runs/apple.research.json
cargo test --locked
~~~

이미 만든 출력 파일은 덮어쓰지 않습니다. 기존 계약 파일을 재사용하거나 새 출력 경로를 지정합니다. 데모의 AI 판단은 **사람이 작성한 설명용 예제**이며 실제 AI 성능 실험이 아닙니다. 로컬 카탈로그 선택은 네트워크를 사용하지 않습니다.

온라인 서지정보 검색은 다음과 같습니다. 검색어가 Crossref로 전송되며 AI API는 호출하지 않습니다.

~~~text
cargo run --locked -- discover local-runs/apple.lock.json --query "surface reflectance illuminant color perception" --rows 10 --out local-runs/apple.search.json
~~~

## OpenAI와 Claude 연결

환경변수 `OPENAI_API_KEY`와 `ANTHROPIC_API_KEY`를 설정하고, 각 API 계정에서 이용할 수 있는 구조화 출력 지원 모델 ID를 지정합니다.

~~~text
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --research local-runs/apple.research.json --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL
~~~

실제 채점은 두 API를 호출하므로 요금이 발생할 수 있습니다. ChatGPT 구독과 API 이용은 별도입니다. 재시도 횟수·시간·출력 토큰·응답 크기는 제한하지만 정확한 금액 상한을 보장하지는 않습니다. 인증키는 계약이나 실행 결과에 저장하지 않습니다.

## 현재 범위와 문서

| 기능 | 상태 |
|---|---|
| Rust CLI와 실제 RelSpec/AHAA 컴파일 | 구현 |
| 고정 판정 항목·근거 검증·두 AI 불일치 처리 | 구현 |
| 현재 타당성과 미래 탐색 점수 분리 | 구현 |
| 실행 기록과 재생 | 구현 |
| 카탈로그 자동 선택·중복 제한·선택 스냅샷 | 구현 |
| Crossref 서지정보 검색 | 구현; 출판물 메타데이터만 검색 |
| 이론의 자동 검증·모든 관점의 완전한 탐색 | 미구현 |
| 기존 채점보다 우수하다는 실증 | 아직 없음 |

- [Rust 실행·계약·API 안내](docs/RUST_GUIDE.md)
- [검색·출처·카탈로그·선택 안내](docs/RESEARCH_GUIDE.md)
- [영문 교재: Rust와 연구 맥락 선택 장 포함](book/Percent-Grading-Textbook.md)
- [구조와 신뢰 경계](docs/ARCHITECTURE.md)
- [실험 계획](docs/EXPERIMENTS.md) · [로드맵](docs/ROADMAP.md)

기존 Python [채점 공식](docs/SCORING.md)과 [입력 규약](docs/PROTOCOL.md)은 0.1.1 교육용 프로토타입 설명입니다. Rust는 그 판단 파일을 자동으로 같은 형식으로 취급하지 않습니다.

프로젝트 고유 코드와 문서는 [MIT License](LICENSE)를 사용합니다. 포함한 RelSpec/AHAA 원본은 [별도 출처 안내](vendor/relspec/UPSTREAM.md)를 따르며, 이 프로젝트의 라이선스로 재허가하지 않습니다. 개념과 프로젝트 제안은 김유빈이 했고, 문서와 구현에는 AI의 도움을 받았습니다.
