# % 채점학 · Percent Grading

**타당한 추론을 인정하면서도, 근거 없는 결론을 드러내는 채점 프로그램입니다.**

[English](README.md) | [한국어](README.ko.md) | [Rust 실행 안내](docs/RUST_GUIDE.md) | [이론 검색·선택 안내](docs/RESEARCH_GUIDE.md) | [근거 검토 안내](docs/EVIDENCE_GUIDE.md) | [평가 실험](docs/BENCHMARK_GUIDE.md) | [앱·웹 연결](docs/API_GUIDE.md) | [영문 교재](book/Percent-Grading-Textbook.md)

제안자: **김유빈(Yubin Kim)**. 현재 구현은 **Rust 0.6.0**이며, 초기 Python 0.1.1 교육용 프로토타입도 보존합니다.

0.6에서는 [과거 자동 채점의 실패 원인 분석](docs/HISTORICAL_FAILURES.md), 취약점을 겨냥한 회귀 평가, [공식 점수 99.9% 상한](docs/SCORE_POLICY.md), [검증 가능한 증명 이스터에그](docs/PROOF_GUIDE.md)를 추가했습니다. **10,000%**는 설명 문구와 함께 나타나는 별도 축하 표시이며, 공식 성적을 바꾸지 않습니다.

## 과거의 실패에서 무엇을 배웠는가

초기 자동 채점에도 통계와 기계학습이 사용되었습니다. 연구에서는 글의 길이, 무관한 설명, 뒤섞인 핵심 단어가 점수를 끌어올리는 문제와 사람의 편향이 학습 정답에 들어가는 문제가 확인됐습니다. 프로그래밍 채점도 준비한 테스트를 통과했다는 사실만으로 모든 동작이 맞다고 보장하지 못합니다. 모든 과거 시스템이 실패했다는 뜻은 아니며, 각 연구가 확인한 조건과 한계를 구분해 기록했습니다.

새로운 **24개 공격·변형 답안**과 고정된 통과 기준으로 전체 정확도, 판단 보류율, 두 AI의 공동 오답, 답안 변형 관계, 유형별 성능을 함께 점검합니다. 제공된 수동 결과는 검사 장치가 오류를 잡는지 확인하도록 일부러 실패하게 만들었습니다. 실제 모델의 성능 측정치는 아닙니다. 자세한 실행법은 [회귀 평가 안내](benchmarks/adversarial/README.md)를 참고하세요.

## 만점 99.9%와 10,000% 발견 연출

새 Rust 채점은 가중 점수를 계산한 뒤 공식 점수에 **99.9% 상한**을 적용합니다. 모든 점수를 0.999배로 줄이지는 않습니다. 항목별 원점수와 평가 완료율은 여전히 100까지 사용할 수 있으며, 과거 저장 결과는 당시의 점수 체계로 재생합니다. 99.9%라는 숫자가 진실일 확률이나 측정된 불확실성을 뜻하지는 않습니다.

출제자가 RelSpec으로 고정한 작은 명제논리 문제에 대해, 답안에 포함된 형식 명제와 완전한 진리표 증명서를 Rust가 직접 검사합니다. 검증을 통과하고 현재 과제의 모든 항목이 실제 만점이며 보류·차단 상태가 없을 때 **10,000%** 축하 표시가 나옵니다. 공식 성적은 **99.9%**로 유지됩니다.

AI는 증명서 작성을 도울 수 있지만, “AI가 증명했다”는 주장이나 두 AI의 동의만으로는 발동하지 않습니다. 첫 검증기는 유한한 명제논리만 지원하며, 페르마 정리 같은 일반 수학 증명이나 새로운 발견을 판정하지 않습니다. [증명 예제 실행하기](docs/PROOF_GUIDE.md).

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

## 채점에 사용할 근거 검토

0.4.0에서는 답안 채점 전에 **주장별 근거와 성립 조건을 검토하는 선택적 절차**를 추가했습니다. 별도의 RelSpec/AHAA 정책에 자료 채택·제외 조건, 검색 계획, 기준 시점, 독립 근거 수, 주장과 채점 항목의 관계를 선언합니다.

- 원문의 어느 부분이 해당 주장을 뒷받침하는지 정확한 인용으로 연결합니다.
- OpenAI와 Claude가 같은 자료를 읽고 지지·반박 관계, 적용 조건, 자료 적격성을 각각 판단합니다.
- Rust는 `지지됨 / 반박됨 / 근거 충돌 / 근거 부족 / 가설` 상태를 계산합니다.
- 필요한 근거가 미해결이면 두 채점 AI가 만점에 동의해도 해당 항목과 관련된 최종 점수를 확정하지 않습니다. 근거 부족을 학생의 오답이나 자동 0점으로 바꾸지 않습니다.
- 근거 정책을 개정하면 등록된 답안 전체를 같은 새 기준으로 재평가할 계획을 만듭니다. 이 계획은 API 호출이나 재채점을 자동 실행하지 않습니다.

```text
cargo run --locked -- evidence-policy local-runs/apple.lock.json policies/apple-evidence.aha --out local-runs/apple.evidence-policy.json
cargo run --locked -- evidence-prepare local-runs/apple.lock.json local-runs/apple.evidence-policy.json examples/evidence/apple.dossier.json --out local-runs/apple.evidence-input.json
cargo run --locked -- evidence-import local-runs/apple.lock.json local-runs/apple.evidence-input.json examples/evidence/openai.review.json examples/evidence/claude.review.json --out local-runs/apple.evidence.json
cargo run --locked -- inspect-evidence local-runs/apple.lock.json local-runs/apple.evidence.json
```

이 예시는 **합성 자료와 사람이 작성한 검토 결과**로 검증 절차를 보여 줍니다. 실제 논문이나 AI의 정확도를 검증했다는 뜻이 아닙니다. 프로그램은 입력한 원문의 인용 일치 여부를 검사하지만 해당 웹페이지를 자동으로 가져와 진위를 인증하지 않습니다. 검색 기록과 자료의 독립성 표기도 작성자가 제공하며, 두 AI가 같은 오류를 범할 가능성도 남습니다.

실제 자료 검토는 `audit-evidence`, 이후 답안 채점은 `grade --evidence`로 실행합니다. 구체적인 영어 사용법과 제한은 [근거 검토 안내](docs/EVIDENCE_GUIDE.md)에 있습니다.

## 채점기를 시험하고 앱·웹에 연결하기

0.5에서는 **10문제·60답안으로 구성한 평가셋**, 일괄 평가 실행기, 오류 보고서를 추가했습니다. 타당한 대안, 부분 정답, 그럴듯한 오답, 조건 변경, 모순, 지시문 삽입 등을 검사합니다. 보고서에는 잘못 확정한 항목, 판정 보류, 두 모델의 잘못된 합의, 표현 변화에 따른 판정 변화와 반복 실험의 안정성이 구분되어 표시됩니다.

```text
cargo run --locked -- benchmark-validate benchmarks/suite.json
cargo run --locked -- benchmark benchmarks/suite.json benchmarks/manual-results.json --out local-runs/benchmark.manual-report.json
```

포함된 결과 파일은 오류 처리도 확인하도록 일부 실패를 넣어 작성한 **수동 예제**입니다. 실제 OpenAI·Claude의 채점 정확도를 측정한 결과가 아닙니다. 개발용 7문제와 검사 용도 3문제를 나누었지만, 공개된 소규모 평가셋 자체가 독립적인 성능 검증을 보장하지는 않습니다.

실제 API 실험은 실행 계획과 최대 호출 시도 수를 먼저 확인한 후 진행합니다. 반복 실험은 같은 답안을 새로운 실험 ID로 평가하며, 중단 후 재개할 때도 기존 응답과 이미 예약한 호출 예산을 유지합니다. 기대 정답이나 허용 앵커는 모델 프롬프트에 포함하지 않습니다. [평가 실험 안내](docs/BENCHMARK_GUIDE.md)에 전체 명령과 해석 방법이 있습니다.

앱·웹에서 재사용할 수 있도록 **Rust 라이브러리, 버전이 있는 JSON API, 로컬 HTTP 서버, JavaScript 클라이언트와 TypeScript 선언**도 제공합니다. 같은 채점 엔진을 호출하므로 화면이나 전송 방식이 바뀌어도 점수 계산 규칙은 유지됩니다.

```text
cargo run --locked --example embed
cargo run --locked -- api examples/integration/compile.request.json
```

위 두 예제는 API 비용 없이 실행됩니다. 로컬 HTTP 서버는 인증 토큰을 요구하며 기본적으로 실제 AI 호출을 허용하지 않습니다. 웹 화면은 자체 백엔드를 통해 연결하고, 서비스 토큰과 제공자 API 키를 공개 프런트엔드에 넣지 않습니다. [앱·웹 연결 안내](docs/API_GUIDE.md)에서 Node.js 예제와 사용 범위를 확인할 수 있습니다.

## OpenAI와 Claude 연결

환경변수 `OPENAI_API_KEY`와 `ANTHROPIC_API_KEY`를 설정하고, 각 API 계정에서 이용할 수 있는 구조화 출력 지원 모델 ID를 지정합니다.

~~~text
cargo run --locked -- grade local-runs/apple.lock.json examples/rust/apple-blue.txt --research local-runs/apple.research.json --openai-model YOUR_OPENAI_MODEL --claude-model YOUR_CLAUDE_MODEL
~~~

실제 채점은 두 API를 호출하므로 요금이 발생할 수 있습니다. ChatGPT 구독과 API 이용은 별도입니다. 재시도 횟수·시간·출력 토큰·응답 크기는 제한하지만 정확한 금액 상한을 보장하지는 않습니다. 인증키는 계약이나 실행 결과에 저장하지 않습니다.

검토된 근거를 사용하려면 `--research` 대신 `--evidence local-runs/apple.evidence.json`을 지정합니다. 두 옵션은 함께 쓸 수 없습니다. `audit-evidence`의 자료 검토와 `grade`의 답안 채점은 별도의 유료 API 호출이며, 저장된 결과의 재생에는 새 호출이 필요하지 않습니다.

## 현재 범위와 문서

| 기능 | 상태 |
|---|---|
| Rust CLI와 실제 RelSpec/AHAA 컴파일 | 구현 |
| 고정 판정 항목·근거 검증·두 AI 불일치 처리 | 구현 |
| 현재 타당성과 미래 탐색 점수 분리 | 구현 |
| 실행 기록과 재생 | 구현 |
| 카탈로그 자동 선택·중복 제한·선택 스냅샷 | 구현 |
| Crossref 서지정보 검색 | 구현; 출판물 메타데이터만 검색 |
| 주장·원문 연결, 두 AI 자료 검토, 근거 미해결 시 점수 확정 보류 | 구현; 의미 판단의 정확성까지 보장하지 않음 |
| 근거 정책 비교와 등록 답안 전체 재평가 계획 | 구현; 실제 재채점은 별도 실행 |
| 이론의 자동 검증·모든 관점의 완전한 탐색 | 미구현 |
| 기존 채점보다 우수하다는 실증 | 아직 없음 |

- [Rust 실행·계약·API 안내](docs/RUST_GUIDE.md)
- [검색·출처·카탈로그·선택 안내](docs/RESEARCH_GUIDE.md)
- [주장별 근거 검토와 공통 재평가 안내](docs/EVIDENCE_GUIDE.md)
- [영문 교재: Rust와 연구 맥락 선택 장 포함](book/Percent-Grading-Textbook.md)
- [구조와 신뢰 경계](docs/ARCHITECTURE.md)
- [실험 계획](docs/EXPERIMENTS.md) · [로드맵](docs/ROADMAP.md)

기존 Python [채점 공식](docs/SCORING.md)과 [입력 규약](docs/PROTOCOL.md)은 0.1.1 교육용 프로토타입 설명입니다. Rust는 그 판단 파일을 자동으로 같은 형식으로 취급하지 않습니다.

프로젝트 고유 코드와 문서는 [MIT License](LICENSE)를 사용합니다. 포함한 RelSpec/AHAA 원본은 [별도 출처 안내](vendor/relspec/UPSTREAM.md)를 따르며, 이 프로젝트의 라이선스로 재허가하지 않습니다. 개념과 프로젝트 제안은 김유빈이 했고, 문서와 구현에는 AI의 도움을 받았습니다.
