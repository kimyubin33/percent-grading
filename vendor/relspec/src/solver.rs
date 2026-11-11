//! 차이 제약(`x - y <= c`)의 최단 경로 폐포.
//!
//! 각 미지값을 꼭짓점으로, 제약 `x - y <= c`를 `y → x` 가중치 `c`인 간선으로 둡니다.
//! 꼭짓점 0은 원점(값 0)입니다. 폐포 `d[y][x]`는 모든 해에서 `x - y`가 가질 수 있는
//! 가장 큰 값이며, 음의 순환이 생기면 해가 없습니다(Simple Temporal Network).
//! 모든 계산은 정수로 하므로 반올림 오차가 없습니다.

use crate::integer::Integer;

pub(crate) const ORIGIN: usize = 0;

#[derive(Clone)]
pub(crate) struct Closure {
    /// `None`은 상한이 없음을 뜻합니다.
    distance: Vec<Vec<Option<Integer>>>,
}

impl Closure {
    pub(crate) fn new() -> Self {
        Self {
            distance: vec![vec![Some(Integer::from_small(0))]],
        }
    }

    pub(crate) fn add_node(&mut self) -> usize {
        let id = self.distance.len();
        for row in &mut self.distance {
            row.push(None);
        }
        let mut row = vec![None; id + 1];
        row[id] = Some(Integer::from_small(0));
        self.distance.push(row);
        id
    }

    /// `to - from`의 상한. 모든 해에서 성립합니다.
    pub(crate) fn upper(&self, from: usize, to: usize) -> Option<&Integer> {
        self.distance[from][to].as_ref()
    }

    /// `to - from <= weight`를 추가합니다. 모순이면 앞선 제약이 요구하는
    /// `from - to`의 상한을 돌려주고 폐포는 바꾸지 않습니다.
    pub(crate) fn add(&mut self, from: usize, to: usize, weight: &Integer) -> Result<(), Integer> {
        if let Some(back) = &self.distance[to][from]
            && back.add(weight) < Integer::from_small(0)
        {
            return Err(back.clone());
        }
        let n = self.distance.len();
        for i in 0..n {
            let Some(to_from) = self.distance[i][from].clone() else {
                continue;
            };
            let head = to_from.add(weight);
            for j in 0..n {
                let Some(tail) = &self.distance[to][j] else {
                    continue;
                };
                let candidate = head.add(tail);
                let cell = &mut self.distance[i][j];
                if cell.as_ref().is_none_or(|current| candidate < *current) {
                    *cell = Some(candidate);
                }
            }
        }
        Ok(())
    }
}
