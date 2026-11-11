//! 정도(퍼지 논리의 참 정도)를 반올림 없이 정확하게 다룹니다.
//!
//! 정도는 `n / 100^scale` 퍼센트입니다. `70%`는 (70, 0), 곱으로 만든 `16.5%`는 (1650, 1)입니다.
//! 언제나 가장 짧은 표현으로 정규화하므로 같은 정도는 같은 (n, scale)을 가집니다.

use std::cmp::Ordering;

use crate::integer::Integer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Degree {
    pub(crate) n: Integer,
    pub(crate) scale: u32,
}

fn hundred() -> Integer {
    Integer::from_small(100)
}

impl Degree {
    pub(crate) fn new(n: Integer, scale: u32) -> Self {
        Self { n, scale }.normalized()
    }

    pub(crate) fn percent(n: u32) -> Self {
        Self::new(Integer::from_small(n), 0)
    }

    fn normalized(mut self) -> Self {
        while self.scale > 0 {
            let (quotient, remainder) = self.n.div_rem_small(100);
            if remainder != 0 {
                break;
            }
            self.n = quotient;
            self.scale -= 1;
        }
        self
    }

    /// `scale` 자리로 맞춘 분자.
    fn at(&self, scale: u32) -> Integer {
        (self.scale..scale).fold(self.n.clone(), |n, _| n.mul(&hundred()))
    }

    pub(crate) fn compare(&self, other: &Self) -> Ordering {
        let scale = self.scale.max(other.scale);
        self.at(scale).cmp(&other.at(scale))
    }

    pub(crate) fn min(self, other: Self) -> Self {
        if self.compare(&other) == Ordering::Greater {
            other
        } else {
            self
        }
    }

    pub(crate) fn max(self, other: Self) -> Self {
        if self.compare(&other) == Ordering::Less {
            other
        } else {
            self
        }
    }

    /// 퍼지 부정: 100% − 정도
    pub(crate) fn complement(&self) -> Self {
        let full = Self::percent(100).at(self.scale);
        Self::new(full.sub(&self.n), self.scale)
    }

    /// 곱 t-노름: (a/100) × (b/100) × 100 퍼센트
    pub(crate) fn product(&self, other: &Self) -> Self {
        Self::new(self.n.mul(&other.n), self.scale + other.scale + 1)
    }

    /// `70%`, `16.5%`, `4.9%`처럼 씁니다.
    pub(crate) fn display(&self) -> String {
        if self.scale == 0 {
            return format!("{}%", self.n);
        }
        let digits = (2 * self.scale) as usize;
        let text = format!("{:0>width$}", self.n.to_string(), width = digits + 1);
        let (whole, fraction) = text.split_at(text.len() - digits);
        format!("{whole}.{}%", fraction.trim_end_matches('0'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_products_and_display() {
        let half = Degree::percent(50);
        let third = Degree::percent(33);
        assert_eq!(third.product(&half).display(), "16.5%");
        assert_eq!(
            Degree::percent(70).product(&Degree::percent(80)).display(),
            "56%"
        );
        assert_eq!(
            Degree::percent(7).product(&Degree::percent(7)).display(),
            "0.49%"
        );
        let tiny = Degree::percent(1)
            .product(&Degree::percent(1))
            .product(&Degree::percent(1));
        assert_eq!(tiny.display(), "0.0001%");
        assert_eq!(third.product(&half).complement().display(), "83.5%");
        assert_eq!(
            Degree::percent(56),
            Degree::percent(70).product(&Degree::percent(80))
        );
        assert_eq!(
            third.product(&half).compare(&Degree::percent(16)),
            Ordering::Greater
        );
    }
}
