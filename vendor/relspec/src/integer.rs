use std::cmp::Ordering;
use std::fmt;

const BASE: u64 = 1_000_000_000;

// Least significant limb first. Zero has no limbs and a positive sign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Integer {
    negative: bool,
    limbs: Vec<u32>,
}

impl Integer {
    pub(crate) fn from_decimal(digits: &str) -> Self {
        let limbs = digits
            .as_bytes()
            .rchunks(9)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0, |number, digit| number * 10 + u32::from(digit - b'0'))
            })
            .collect();
        Self::normalized(false, limbs)
    }

    pub(crate) fn from_small(number: u32) -> Self {
        let number = u64::from(number);
        Self::normalized(false, vec![(number % BASE) as u32, (number / BASE) as u32])
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub(crate) fn negate(mut self) -> Self {
        self.negative = !self.negative && !self.is_zero();
        self
    }

    pub(crate) fn add(&self, rhs: &Self) -> Self {
        self.add_signed(rhs, rhs.negative)
    }

    pub(crate) fn sub(&self, rhs: &Self) -> Self {
        self.add_signed(rhs, !rhs.negative)
    }

    pub(crate) fn mul(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::from_small(0);
        }

        let mut limbs = vec![0; self.limbs.len() + rhs.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0;
            for (j, &b) in rhs.limbs.iter().enumerate() {
                // One limb product plus one existing limb and a carry fits in u64.
                let partial = u64::from(a) * u64::from(b) + u64::from(limbs[i + j]) + carry;
                limbs[i + j] = (partial % BASE) as u32;
                carry = partial / BASE;
            }
            limbs[i + rhs.limbs.len()] = carry as u32;
        }
        Self::normalized(self.negative ^ rhs.negative, limbs)
    }

    /// 작은 수로 나눈 몫과 나머지. 음이 아닌 수에만 씁니다.
    pub(crate) fn div_rem_small(&self, divisor: u32) -> (Self, u32) {
        debug_assert!(!self.negative && divisor > 0);
        let divisor = u64::from(divisor);
        let mut remainder = 0u64;
        let mut limbs = vec![0; self.limbs.len()];
        for (i, &limb) in self.limbs.iter().enumerate().rev() {
            let current = remainder * BASE + u64::from(limb);
            limbs[i] = (current / divisor) as u32;
            remainder = current % divisor;
        }
        (Self::normalized(false, limbs), remainder as u32)
    }

    fn normalized(negative: bool, mut limbs: Vec<u32>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self {
            negative: negative && !limbs.is_empty(),
            limbs,
        }
    }

    fn compare_magnitude(&self, rhs: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&rhs.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(rhs.limbs.iter().rev()))
    }

    fn add_signed(&self, rhs: &Self, rhs_negative: bool) -> Self {
        if self.negative != rhs_negative {
            return match self.compare_magnitude(rhs) {
                Ordering::Greater => Self::difference(self, rhs, self.negative),
                Ordering::Less => Self::difference(rhs, self, rhs_negative),
                Ordering::Equal => Self::from_small(0),
            };
        }

        let length = self.limbs.len().max(rhs.limbs.len());
        let mut limbs = Vec::with_capacity(length + 1);
        let mut carry = 0;
        for i in 0..length {
            let a = u64::from(self.limbs.get(i).copied().unwrap_or(0));
            let b = u64::from(rhs.limbs.get(i).copied().unwrap_or(0));
            let sum = a + b + carry;
            limbs.push((sum % BASE) as u32);
            carry = sum / BASE;
        }
        limbs.push(carry as u32);
        Self::normalized(self.negative, limbs)
    }

    fn difference(larger: &Self, smaller: &Self, negative: bool) -> Self {
        let mut borrow = 0;
        let mut limbs = Vec::with_capacity(larger.limbs.len());
        for (i, &a) in larger.limbs.iter().enumerate() {
            let b = smaller.limbs.get(i).copied().unwrap_or(0);
            let mut difference = i64::from(a) - i64::from(b) - borrow;
            borrow = i64::from(difference < 0);
            difference += borrow * BASE as i64;
            limbs.push(difference as u32);
        }
        Self::normalized(negative, limbs)
    }
}

impl Ord for Integer {
    fn cmp(&self, rhs: &Self) -> Ordering {
        match (self.negative, rhs.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => self.compare_magnitude(rhs).reverse(),
            (false, false) => self.compare_magnitude(rhs),
        }
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((highest, rest)) = self.limbs.split_last() else {
            return f.write_str("0");
        };
        if self.negative {
            f.write_str("-")?;
        }
        write!(f, "{highest}")?;
        for limb in rest.iter().rev() {
            write!(f, "{limb:09}")?;
        }
        Ok(())
    }
}
