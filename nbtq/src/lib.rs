use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{Debug, Display},
};

pub use anyhow::Error;
use anyhow::anyhow;
use jaq_core::{
    DataT, Exn, RunPtr,
    box_iter::box_once,
    load,
    native::{Filter, Fun, bome, v},
    ops,
};

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq)]
pub struct Val(pub fastnbt::Value);

impl Val {
    pub fn discriminant(&self) -> usize {
        match self.0 {
            fastnbt::Value::Byte(_) => 0,
            fastnbt::Value::Short(_) => 1,
            fastnbt::Value::Int(_) => 2,
            fastnbt::Value::Long(_) => 3,
            fastnbt::Value::Float(_) => 4,
            fastnbt::Value::Double(_) => 5,
            fastnbt::Value::String(_) => 6,
            fastnbt::Value::ByteArray(_) => 7,
            fastnbt::Value::IntArray(_) => 8,
            fastnbt::Value::LongArray(_) => 9,
            fastnbt::Value::List(_) => 10,
            fastnbt::Value::Compound(_) => 11,
        }
    }

    pub fn len(&self) -> Option<usize> {
        match &self.0 {
            fastnbt::Value::String(s) => Some(s.len()),
            fastnbt::Value::ByteArray(s) => Some(s.len()),
            fastnbt::Value::IntArray(s) => Some(s.len()),
            fastnbt::Value::LongArray(s) => Some(s.len()),
            fastnbt::Value::List(s) => Some(s.len()),
            fastnbt::Value::Compound(s) => Some(s.len()),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len().is_some_and(|len| len == 0)
    }
}

pub trait NbtExtension {
    fn maybe_i64(&self) -> Result<i64>;

    fn maybe_usize(&self) -> Result<usize> {
        Ok(usize::try_from(self.maybe_i64()?)?)
    }
}

impl NbtExtension for fastnbt::Value {
    fn maybe_i64(&self) -> Result<i64> {
        match self {
            fastnbt::Value::Byte(v) => Ok(*v as i64),
            fastnbt::Value::Short(v) => Ok(*v as i64),
            fastnbt::Value::Int(v) => Ok(*v as i64),
            fastnbt::Value::Long(v) => Ok(*v),
            fastnbt::Value::Float(v) => {
                if *v == (*v as i64) as f32 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f32 conversion would be lossy"))
                }
            }
            fastnbt::Value::Double(v) => {
                if *v == (*v as i64) as f64 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f64 conversion would be lossy"))
                }
            }
            _ => Err(anyhow!("not a numeric type")),
        }
    }
}

impl NbtExtension for Val {
    fn maybe_i64(&self) -> Result<i64> {
        self.0.maybe_i64()
    }
}

pub type ValR = jaq_core::ValR<Val>;
pub type ValX<'a> = jaq_core::ValX<'a, Val>;

// this is incorrect, but necessary for implementing jaq_std::ValT
impl Eq for Val {}

// this is incorrect, but necessary for implementing jaq_core::ValT
impl Ord for Val {
    fn cmp(&self, other: &Self) -> Ordering {
        use fastnbt::Value::*;

        let discriminant = self.discriminant().cmp(&other.discriminant());
        if discriminant != Ordering::Equal {
            return discriminant;
        }

        match (&self.0, &other.0) {
            (Byte(a), Byte(b)) => a.cmp(b),
            (Short(a), Short(b)) => a.cmp(b),
            (Int(a), Int(b)) => a.cmp(b),
            (Long(a), Long(b)) => a.cmp(b),
            (Float(a), Float(b)) => a.total_cmp(b),
            (Double(a), Double(b)) => a.total_cmp(b),
            (String(a), String(b)) => a.cmp(b),
            (ByteArray(a), ByteArray(b)) => a.cmp(b),
            (IntArray(a), IntArray(b)) => a.cmp(b),
            (LongArray(a), LongArray(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Val {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct DisplayAsDebug<T: Display>(pub T);

impl<T: Display> Debug for DisplayAsDebug<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <T as Display>::fmt(&self.0, f)
    }
}

impl std::fmt::Display for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            fastnbt::Value::Byte(v) => write!(f, "{v}"),
            fastnbt::Value::Short(v) => write!(f, "{v}"),
            fastnbt::Value::Int(v) => write!(f, "{v}"),
            fastnbt::Value::Long(v) => write!(f, "{v}"),
            fastnbt::Value::Float(v) => write!(f, "{v}"),
            fastnbt::Value::Double(v) => write!(f, "{v}"),
            fastnbt::Value::String(v) => write!(f, "{v:?}"),
            fastnbt::Value::ByteArray(v) => write!(f, "{:?}", v.as_ref()),
            fastnbt::Value::IntArray(v) => write!(f, "{:?}", v.as_ref()),
            fastnbt::Value::LongArray(v) => write!(f, "{:?}", v.as_ref()),
            fastnbt::Value::List(v) => f
                .debug_list()
                .entries(v.iter().cloned().map(Val).map(DisplayAsDebug))
                .finish(),
            fastnbt::Value::Compound(v) => f
                .debug_map()
                .entries(
                    v.iter()
                        .map(|(k, v)| (DisplayAsDebug(k), DisplayAsDebug(Val(v.clone())))),
                )
                .finish(),
        }
    }
}

impl From<bool> for Val {
    fn from(value: bool) -> Self {
        Val(fastnbt::Value::Byte(if value { 1 } else { 0 }))
    }
}

impl From<jaq_core::val::Range<Self>> for Val {
    fn from(value: jaq_core::val::Range<Self>) -> Self {
        let mut map = HashMap::with_capacity(2);
        value
            .start
            .and_then(|v| map.insert("start".to_string(), v.0));
        value.end.and_then(|v| map.insert("end".to_string(), v.0));
        Val(fastnbt::Value::Compound(map))
    }
}

impl From<isize> for Val {
    fn from(value: isize) -> Self {
        Val(fastnbt::Value::Int(value as i32))
    }
}

impl From<usize> for Val {
    fn from(value: usize) -> Self {
        if value > isize::MAX as usize {
            Val(fastnbt::Value::Long(value as i64))
        } else {
            Val(fastnbt::Value::Int(value as i32))
        }
    }
}

impl From<f64> for Val {
    fn from(value: f64) -> Self {
        Val(fastnbt::Value::Double(value))
    }
}

impl From<String> for Val {
    fn from(value: String) -> Self {
        Val(fastnbt::Value::String(value))
    }
}

impl FromIterator<Self> for Val {
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        Val(fastnbt::Value::List(
            iter.into_iter().map(|v| v.0).collect(),
        ))
    }
}

impl core::ops::Add for Val {
    type Output = ValR;

    fn add(self, rhs: Self) -> Self::Output {
        use fastnbt::Value::{
            Byte, ByteArray, Compound, Double, Float, Int, IntArray, List, Long, LongArray, Short,
            String as Str,
        };
        use jaq_core::Error;

        Ok(Val(match (self.0, rhs.0) {
            (Byte(a), Byte(b)) => Short(a as i16 + b as i16),
            (Byte(a), Short(b)) => Int(a as i32 + b as i32),
            (Byte(a), Int(b)) => Long(a as i64 + b as i64),
            (Byte(a), Long(b)) => Long(a as i64 + b),
            (Byte(a), Float(b)) => Double(a as f64 + b as f64),
            (Byte(a), Double(b)) => Double(a as f64 + b),
            (Short(a), Byte(b)) => Int(a as i32 + b as i32),
            (Short(a), Short(b)) => Int(a as i32 + b as i32),
            (Short(a), Int(b)) => Long(a as i64 + b as i64),
            (Short(a), Long(b)) => Long(a as i64 + b),
            (Short(a), Float(b)) => Double(a as f64 + b as f64),
            (Short(a), Double(b)) => Double(a as f64 + b),
            (Int(a), Byte(b)) => Long(a as i64 + b as i64),
            (Int(a), Short(b)) => Long(a as i64 + b as i64),
            (Int(a), Int(b)) => Long(a as i64 + b as i64),
            (Int(a), Long(b)) => Long(a as i64 + b),
            (Int(a), Float(b)) => Double(a as f64 + b as f64),
            (Int(a), Double(b)) => Double(a as f64 + b),
            (Long(a), Byte(b)) => Long(a + b as i64),
            (Long(a), Short(b)) => Long(a + b as i64),
            (Long(a), Int(b)) => Long(a + b as i64),
            (Long(a), Long(b)) => Long(a + b),
            (Long(a), Float(b)) => Double(a as f64 + b as f64),
            (Long(a), Double(b)) => Double(a as f64 + b),
            (Float(a), Byte(b)) => Double(a as f64 + b as f64),
            (Float(a), Short(b)) => Double(a as f64 + b as f64),
            (Float(a), Int(b)) => Double(a as f64 + b as f64),
            (Float(a), Long(b)) => Double(a as f64 + b as f64),
            (Float(a), Float(b)) => Double(a as f64 + b as f64),
            (Float(a), Double(b)) => Double(a as f64 + b),
            (Double(a), Byte(b)) => Double(a + b as f64),
            (Double(a), Short(b)) => Double(a + b as f64),
            (Double(a), Int(b)) => Double(a + b as f64),
            (Double(a), Long(b)) => Double(a + b as f64),
            (Double(a), Float(b)) => Double(a + b as f64),
            (Double(a), Double(b)) => Double(a + b),
            (Str(a), Str(b)) => {
                let mut s = String::with_capacity(a.len() + b.len());
                s.push_str(&a);
                s.push_str(&b);
                Str(s)
            }
            (List(a), List(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
                List(v)
            }
            (List(a), ByteArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(Byte));
                List(v)
            }
            (ByteArray(a), List(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(Byte));
                v.extend_from_slice(&b);
                List(v)
            }
            (List(a), IntArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(Int));
                List(v)
            }
            (IntArray(a), List(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(Int));
                v.extend_from_slice(&b);
                List(v)
            }
            (List(a), LongArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(Long));
                List(v)
            }
            (LongArray(a), List(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(Long));
                v.extend_from_slice(&b);
                List(v)
            }
            (ByteArray(a), ByteArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
                ByteArray(fastnbt::ByteArray::new(v))
            }
            (ByteArray(a), IntArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(|n| n as i32));
                v.extend_from_slice(&b);
                IntArray(fastnbt::IntArray::new(v))
            }
            (ByteArray(a), LongArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(|n| n as i64));
                v.extend_from_slice(&b);
                LongArray(fastnbt::LongArray::new(v))
            }
            (IntArray(a), ByteArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(|n| n as i32));
                IntArray(fastnbt::IntArray::new(v))
            }
            (IntArray(a), IntArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
                IntArray(fastnbt::IntArray::new(v))
            }
            (IntArray(a), LongArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend(a.iter().copied().map(|n| n as i64));
                v.extend_from_slice(&b);
                LongArray(fastnbt::LongArray::new(v))
            }
            (LongArray(a), ByteArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(|n| n as i64));
                LongArray(fastnbt::LongArray::new(v))
            }
            (LongArray(a), IntArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend(b.iter().copied().map(|n| n as i64));
                LongArray(fastnbt::LongArray::new(v))
            }
            (LongArray(a), LongArray(b)) => {
                let mut v = Vec::with_capacity(a.len() + b.len());
                v.extend_from_slice(&a);
                v.extend_from_slice(&b);
                LongArray(fastnbt::LongArray::new(v))
            }
            (Compound(a), Compound(b)) => {
                let mut map = a.clone();
                map.reserve(b.len());
                map.extend(b);
                Compound(map)
            }
            (l, r) => return Err(Error::math(Val(l), ops::Math::Add, Val(r))),
        }))
    }
}

impl core::ops::Sub for Val {
    type Output = ValR;
    fn sub(self, rhs: Self) -> Self::Output {
        use fastnbt::Value::{
            Byte, ByteArray, Double, Float, Int, IntArray, List, Long, LongArray, Short,
        };
        use jaq_core::Error;

        Ok(Val(match (self.0, rhs.0) {
            (Byte(a), Byte(b)) => Short(a as i16 - b as i16),
            (Byte(a), Short(b)) => Int(a as i32 - b as i32),
            (Byte(a), Int(b)) => Long(a as i64 - b as i64),
            (Byte(a), Long(b)) => Long(a as i64 - b),
            (Byte(a), Float(b)) => Double(a as f64 - b as f64),
            (Byte(a), Double(b)) => Double(a as f64 - b),
            (Short(a), Byte(b)) => Int(a as i32 - b as i32),
            (Short(a), Short(b)) => Int(a as i32 - b as i32),
            (Short(a), Int(b)) => Long(a as i64 - b as i64),
            (Short(a), Long(b)) => Long(a as i64 - b),
            (Short(a), Float(b)) => Double(a as f64 - b as f64),
            (Short(a), Double(b)) => Double(a as f64 - b),
            (Int(a), Byte(b)) => Long(a as i64 - b as i64),
            (Int(a), Short(b)) => Long(a as i64 - b as i64),
            (Int(a), Int(b)) => Long(a as i64 - b as i64),
            (Int(a), Long(b)) => Long(a as i64 - b),
            (Int(a), Float(b)) => Double(a as f64 - b as f64),
            (Int(a), Double(b)) => Double(a as f64 - b),
            (Long(a), Byte(b)) => Long(a - b as i64),
            (Long(a), Short(b)) => Long(a - b as i64),
            (Long(a), Int(b)) => Long(a - b as i64),
            (Long(a), Long(b)) => Long(a - b),
            (Long(a), Float(b)) => Double(a as f64 - b as f64),
            (Long(a), Double(b)) => Double(a as f64 - b),
            (Float(a), Byte(b)) => Double(a as f64 - b as f64),
            (Float(a), Short(b)) => Double(a as f64 - b as f64),
            (Float(a), Int(b)) => Double(a as f64 - b as f64),
            (Float(a), Long(b)) => Double(a as f64 - b as f64),
            (Float(a), Float(b)) => Double(a as f64 - b as f64),
            (Float(a), Double(b)) => Double(a as f64 - b),
            (Double(a), Byte(b)) => Double(a - b as f64),
            (Double(a), Short(b)) => Double(a - b as f64),
            (Double(a), Int(b)) => Double(a - b as f64),
            (Double(a), Long(b)) => Double(a - b as f64),
            (Double(a), Float(b)) => Double(a - b as f64),
            (Double(a), Double(b)) => Double(a - b),
            (List(a), List(b)) => List(a.iter().filter(|v| !b.contains(v)).cloned().collect()),
            (List(a), ByteArray(b)) => {
                let b = b.iter().copied().map(Byte).collect::<Vec<_>>();
                List(a.iter().filter(|v| !b.contains(v)).cloned().collect())
            }
            (ByteArray(a), List(b)) => List(
                a.iter()
                    .copied()
                    .map(Byte)
                    .filter(|v| !b.contains(v))
                    .collect(),
            ),
            (List(a), IntArray(b)) => {
                let b = b.iter().copied().map(Int).collect::<Vec<_>>();
                List(a.iter().filter(|v| !b.contains(v)).cloned().collect())
            }
            (IntArray(a), List(b)) => List(
                a.iter()
                    .copied()
                    .map(Int)
                    .filter(|v| !b.contains(v))
                    .collect(),
            ),
            (List(a), LongArray(b)) => {
                let b = b.iter().copied().map(Long).collect::<Vec<_>>();
                List(a.iter().filter(|v| !b.contains(v)).cloned().collect())
            }
            (LongArray(a), List(b)) => List(
                a.iter()
                    .copied()
                    .map(Long)
                    .filter(|v| !b.contains(v))
                    .collect(),
            ),
            (ByteArray(a), ByteArray(b)) => ByteArray(fastnbt::ByteArray::new(
                a.iter().filter(|v| !b.contains(v)).cloned().collect(),
            )),
            (ByteArray(a), IntArray(b)) => {
                let b = b
                    .iter()
                    .copied()
                    .filter_map(|n| i8::try_from(n).ok())
                    .collect::<Vec<_>>();
                ByteArray(fastnbt::ByteArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (ByteArray(a), LongArray(b)) => {
                let b = b
                    .iter()
                    .copied()
                    .filter_map(|n| i8::try_from(n).ok())
                    .collect::<Vec<_>>();
                ByteArray(fastnbt::ByteArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (IntArray(a), ByteArray(b)) => {
                let b = b.iter().copied().map(|n| n as i32).collect::<Vec<_>>();
                IntArray(fastnbt::IntArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (IntArray(a), IntArray(b)) => IntArray(fastnbt::IntArray::new(
                a.iter().filter(|v| !b.contains(v)).copied().collect(),
            )),
            (IntArray(a), LongArray(b)) => {
                let b = b
                    .iter()
                    .copied()
                    .filter_map(|n| i32::try_from(n).ok())
                    .collect::<Vec<_>>();
                IntArray(fastnbt::IntArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (LongArray(a), ByteArray(b)) => {
                let b = b.iter().copied().map(|n| n as i64).collect::<Vec<_>>();
                LongArray(fastnbt::LongArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (LongArray(a), IntArray(b)) => {
                let b = b.iter().copied().map(|n| n as i64).collect::<Vec<_>>();
                LongArray(fastnbt::LongArray::new(
                    a.iter().filter(|v| !b.contains(v)).copied().collect(),
                ))
            }
            (LongArray(a), LongArray(b)) => LongArray(fastnbt::LongArray::new(
                a.iter().filter(|v| !b.contains(v)).copied().collect(),
            )),
            (l, r) => return Err(Error::math(Val(l), ops::Math::Sub, Val(r))),
        }))
    }
}

impl core::ops::Mul for Val {
    type Output = ValR;
    fn mul(self, rhs: Self) -> Self::Output {
        use fastnbt::Value::{Byte, Compound, Double, Float, Int, Long, Short, String as Str};
        use jaq_core::Error;

        Ok(Val(match (self.0, rhs.0) {
            (Byte(a), Byte(b)) => Short(a as i16 * b as i16),
            (Byte(a), Short(b)) => Int(a as i32 * b as i32),
            (Byte(a), Int(b)) => Long(a as i64 * b as i64),
            (Byte(a), Long(b)) => Long(a as i64 * b),
            (Byte(a), Float(b)) => Double(a as f64 * b as f64),
            (Byte(a), Double(b)) => Double(a as f64 * b),
            (Short(a), Byte(b)) => Int(a as i32 * b as i32),
            (Short(a), Short(b)) => Int(a as i32 * b as i32),
            (Short(a), Int(b)) => Long(a as i64 * b as i64),
            (Short(a), Long(b)) => Long(a as i64 * b),
            (Short(a), Float(b)) => Double(a as f64 * b as f64),
            (Short(a), Double(b)) => Double(a as f64 * b),
            (Int(a), Byte(b)) => Long(a as i64 * b as i64),
            (Int(a), Short(b)) => Long(a as i64 * b as i64),
            (Int(a), Int(b)) => Long(a as i64 * b as i64),
            (Int(a), Long(b)) => Long(a as i64 * b),
            (Int(a), Float(b)) => Double(a as f64 * b as f64),
            (Int(a), Double(b)) => Double(a as f64 * b),
            (Long(a), Byte(b)) => Long(a * b as i64),
            (Long(a), Short(b)) => Long(a * b as i64),
            (Long(a), Int(b)) => Long(a * b as i64),
            (Long(a), Long(b)) => Long(a * b),
            (Long(a), Float(b)) => Double(a as f64 * b as f64),
            (Long(a), Double(b)) => Double(a as f64 * b),
            (Float(a), Byte(b)) => Double(a as f64 * b as f64),
            (Float(a), Short(b)) => Double(a as f64 * b as f64),
            (Float(a), Int(b)) => Double(a as f64 * b as f64),
            (Float(a), Long(b)) => Double(a as f64 * b as f64),
            (Float(a), Float(b)) => Double(a as f64 * b as f64),
            (Float(a), Double(b)) => Double(a as f64 * b),
            (Double(a), Byte(b)) => Double(a * b as f64),
            (Double(a), Short(b)) => Double(a * b as f64),
            (Double(a), Int(b)) => Double(a * b as f64),
            (Double(a), Long(b)) => Double(a * b as f64),
            (Double(a), Float(b)) => Double(a * b as f64),
            (Double(a), Double(b)) => Double(a * b),
            (Str(s), n) if let Ok(n) = n.maybe_usize() => Str(s.repeat(n)),
            (n, Str(s)) if let Ok(n) = n.maybe_usize() => Str(s.repeat(n)),
            (Compound(a), Compound(b)) => {
                let mut map = a.clone();
                map.extend(b);
                Compound(map)
            }
            (l, r) => return Err(Error::math(Val(l), ops::Math::Mul, Val(r))),
        }))
    }
}

impl core::ops::Div for Val {
    type Output = ValR;
    fn div(self, rhs: Self) -> Self::Output {
        use fastnbt::Value::{Byte, Double, Float, Int, Long, Short};
        use jaq_core::Error;

        Ok(Val(match (self.0, rhs.0) {
            (Byte(a), Byte(b)) => Double(a as f64 / b as f64),
            (Byte(a), Short(b)) => Double(a as f64 / b as f64),
            (Byte(a), Int(b)) => Double(a as f64 / b as f64),
            (Byte(a), Long(b)) => Double(a as f64 / b as f64),
            (Byte(a), Float(b)) => Double(a as f64 / b as f64),
            (Byte(a), Double(b)) => Double(a as f64 / b),
            (Short(a), Byte(b)) => Double(a as f64 / b as f64),
            (Short(a), Short(b)) => Double(a as f64 / b as f64),
            (Short(a), Int(b)) => Double(a as f64 / b as f64),
            (Short(a), Long(b)) => Double(a as f64 / b as f64),
            (Short(a), Float(b)) => Double(a as f64 / b as f64),
            (Short(a), Double(b)) => Double(a as f64 / b),
            (Int(a), Byte(b)) => Double(a as f64 / b as f64),
            (Int(a), Short(b)) => Double(a as f64 / b as f64),
            (Int(a), Int(b)) => Double(a as f64 / b as f64),
            (Int(a), Long(b)) => Double(a as f64 / b as f64),
            (Int(a), Float(b)) => Double(a as f64 / b as f64),
            (Int(a), Double(b)) => Double(a as f64 / b),
            (Long(a), Byte(b)) => Double(a as f64 / b as f64),
            (Long(a), Short(b)) => Double(a as f64 / b as f64),
            (Long(a), Int(b)) => Double(a as f64 / b as f64),
            (Long(a), Long(b)) => Double(a as f64 / b as f64),
            (Long(a), Float(b)) => Double(a as f64 / b as f64),
            (Long(a), Double(b)) => Double(a as f64 / b),
            (Float(a), Byte(b)) => Double(a as f64 / b as f64),
            (Float(a), Short(b)) => Double(a as f64 / b as f64),
            (Float(a), Int(b)) => Double(a as f64 / b as f64),
            (Float(a), Long(b)) => Double(a as f64 / b as f64),
            (Float(a), Float(b)) => Double(a as f64 / b as f64),
            (Float(a), Double(b)) => Double(a as f64 / b),
            (Double(a), Byte(b)) => Double(a / b as f64),
            (Double(a), Short(b)) => Double(a / b as f64),
            (Double(a), Int(b)) => Double(a / b as f64),
            (Double(a), Long(b)) => Double(a / b as f64),
            (Double(a), Float(b)) => Double(a / b as f64),
            (Double(a), Double(b)) => Double(a / b),
            (l, r) => return Err(Error::math(Val(l), ops::Math::Div, Val(r))),
        }))
    }
}

impl core::ops::Rem for Val {
    type Output = ValR;
    fn rem(self, rhs: Self) -> Self::Output {
        use fastnbt::Value::{Byte, Double, Float, Int, Long, Short};
        use jaq_core::Error;

        Ok(Val(match (self.0, rhs.0) {
            (Byte(a), Byte(b)) => Short(a as i16 % b as i16),
            (Byte(a), Short(b)) => Int(a as i32 % b as i32),
            (Byte(a), Int(b)) => Long(a as i64 % b as i64),
            (Byte(a), Long(b)) => Long(a as i64 % b),
            (Byte(a), Float(b)) => Double(a as f64 % b as f64),
            (Byte(a), Double(b)) => Double(a as f64 % b),
            (Short(a), Byte(b)) => Int(a as i32 % b as i32),
            (Short(a), Short(b)) => Int(a as i32 % b as i32),
            (Short(a), Int(b)) => Long(a as i64 % b as i64),
            (Short(a), Long(b)) => Long(a as i64 % b),
            (Short(a), Float(b)) => Double(a as f64 % b as f64),
            (Short(a), Double(b)) => Double(a as f64 % b),
            (Int(a), Byte(b)) => Long(a as i64 % b as i64),
            (Int(a), Short(b)) => Long(a as i64 % b as i64),
            (Int(a), Int(b)) => Long(a as i64 % b as i64),
            (Int(a), Long(b)) => Long(a as i64 % b),
            (Int(a), Float(b)) => Double(a as f64 % b as f64),
            (Int(a), Double(b)) => Double(a as f64 % b),
            (Long(a), Byte(b)) => Long(a % b as i64),
            (Long(a), Short(b)) => Long(a % b as i64),
            (Long(a), Int(b)) => Long(a % b as i64),
            (Long(a), Long(b)) => Long(a % b),
            (Long(a), Float(b)) => Double(a as f64 % b as f64),
            (Long(a), Double(b)) => Double(a as f64 % b),
            (Float(a), Byte(b)) => Double(a as f64 % b as f64),
            (Float(a), Short(b)) => Double(a as f64 % b as f64),
            (Float(a), Int(b)) => Double(a as f64 % b as f64),
            (Float(a), Long(b)) => Double(a as f64 % b as f64),
            (Float(a), Float(b)) => Double(a as f64 % b as f64),
            (Float(a), Double(b)) => Double(a as f64 % b),
            (Double(a), Byte(b)) => Double(a % b as f64),
            (Double(a), Short(b)) => Double(a % b as f64),
            (Double(a), Int(b)) => Double(a % b as f64),
            (Double(a), Long(b)) => Double(a % b as f64),
            (Double(a), Float(b)) => Double(a % b as f64),
            (Double(a), Double(b)) => Double(a % b),
            (l, r) => return Err(Error::math(Val(l), ops::Math::Rem, Val(r))),
        }))
    }
}

impl core::ops::Neg for Val {
    type Output = ValR;
    fn neg(self) -> Self::Output {
        use fastnbt::Value::{Byte, Double, Float, Int, Long, Short};
        use jaq_core::Error;

        Ok(Val(match self.0 {
            Byte(n) => Byte(-n),
            Short(n) => Short(-n),
            Int(n) => Int(-n),
            Long(n) => Long(-n),
            Float(n) => Float(-n),
            Double(n) => Double(-n),
            n => return Err(Error::typ(Val(n), "number")),
        }))
    }
}

pub fn defs() -> impl Iterator<Item = load::parse::Def<&'static str>> {
    load::parse(include_str!("defs.jq"), |p| p.defs())
        .unwrap()
        .into_iter()
}

pub fn funs<D: for<'a> DataT<V<'a> = Val>>() -> impl Iterator<Item = Fun<D>> {
    base().into_vec().into_iter().map(jaq_core::native::run)
}

fn base<D: for<'a> DataT<V<'a> = Val>>() -> Box<[Filter<RunPtr<D>>]> {
    use fastnbt::Value::*;
    use jaq_core::Error;
    Box::new([("length", v(0), |cv| {
        bome(
            cv.1.len()
                .map(|n| Val(Int(n as i32)))
                .ok_or_else(|| Error::str("no length")),
        )
    })])
}

impl jaq_core::ValT for Val {
    fn from_num(n: &str) -> jaq_core::ValR<Self> {
        use fastnbt::Value::{Byte, Double, Int, Long, Short, String};

        if let Ok(n) = n.parse::<i64>() {
            return Ok(Val(if let Ok(n) = i8::try_from(n) {
                Byte(n)
            } else if let Ok(n) = i16::try_from(n) {
                Short(n)
            } else if let Ok(n) = i32::try_from(n) {
                Int(n)
            } else {
                Long(n)
            }));
        }

        if let Ok(n) = n.parse::<f64>() {
            return Ok(Val(Double(n)));
        }

        Ok(Val(String(n.to_string())))
    }

    fn from_map<I: IntoIterator<Item = (Self, Self)>>(iter: I) -> jaq_core::ValR<Self> {
        use fastnbt::Value::{Compound, String};
        use jaq_core::Error;

        let mut map = HashMap::new();
        for (k, v) in iter {
            let String(k) = k.0 else {
                return Err(Error::typ(k, "String"));
            };
            map.insert(k, v.0);
        }

        Ok(Val(Compound(map)))
    }

    fn key_values(
        self,
    ) -> jaq_core::box_iter::BoxIter<'static, jaq_core::ValR<(Self, Self), Self>> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        match self.0 {
            ByteArray(v) => Box::new(
                v.iter()
                    .copied()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(Byte(n)))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            IntArray(v) => Box::new(
                v.iter()
                    .copied()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(Int(n)))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            LongArray(v) => Box::new(
                v.iter()
                    .copied()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(Long(n)))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            List(v) => Box::new(
                v.iter()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(n.clone()))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            Compound(v) => Box::new(
                v.iter()
                    .map(|(k, v)| Ok((Val(String(k.to_string())), Val(v.clone()))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            _ => box_once(Err(Error::typ(self, "Iter"))),
        }
    }

    fn values(self) -> Box<dyn Iterator<Item = jaq_core::ValR<Self>>> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        match self.0 {
            ByteArray(v) => Box::new(
                v.iter()
                    .copied()
                    .map(Byte)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            IntArray(v) => Box::new(
                v.iter()
                    .copied()
                    .map(Int)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            LongArray(v) => Box::new(
                v.iter()
                    .copied()
                    .map(Long)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            List(v) => Box::new(
                v.iter()
                    .cloned()
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            Compound(v) => Box::new(
                v.values()
                    .cloned()
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            _ => box_once(Err(Error::typ(self, "Iter"))),
        }
    }

    fn index(self, index: &Self) -> jaq_core::ValR<Self> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        match (self.0, index) {
            (ByteArray(v), idx) if let Ok(idx) = idx.maybe_usize() => v
                .get(idx)
                .copied()
                .map(Byte)
                .map(Val)
                .ok_or_else(|| Error::index(Val(ByteArray(v)), Val(Int(idx as i32)))),
            (IntArray(v), idx) if let Ok(idx) = idx.maybe_usize() => v
                .get(idx)
                .copied()
                .map(Int)
                .map(Val)
                .ok_or_else(|| Error::index(Val(IntArray(v)), Val(Int(idx as i32)))),
            (LongArray(v), idx) if let Ok(idx) = idx.maybe_usize() => v
                .get(idx)
                .copied()
                .map(Long)
                .map(Val)
                .ok_or_else(|| Error::index(Val(LongArray(v)), Val(Int(idx as i32)))),
            (List(v), idx) if let Ok(idx) = idx.maybe_usize() => v
                .get(idx)
                .cloned()
                .map(Val)
                .ok_or_else(|| Error::index(Val(List(v)), Val(Int(idx as i32)))),
            (Compound(map), Val(String(k))) => map
                .get(k)
                .cloned()
                .map(Val)
                .ok_or_else(|| Error::index(Val(Compound(map)), Val(String(k.to_string())))),
            (o, k) => Err(Error::index(Val(o), k.clone())),
        }
    }

    fn range(self, range: jaq_core::val::Range<&Self>) -> jaq_core::ValR<Self> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        let size = match &self.0 {
            ByteArray(v) => v.len(),
            IntArray(v) => v.len(),
            LongArray(v) => v.len(),
            List(v) => v.len(),
            _ => return Err(Error::typ(self, "List")),
        };

        let start = range
            .start
            .map(|start| start.0.maybe_i64())
            .unwrap_or(Ok(0))
            .map(|n| {
                if n < 0 {
                    (size as i64 + n) as usize
                } else {
                    n as usize
                }
            })
            .map_err(Error::str)?;

        let end = range
            .end
            .map(|end| end.0.maybe_i64())
            .unwrap_or(Ok(size as i64))
            .map(|n| {
                if n < 0 {
                    (size as i64 + n) as usize
                } else {
                    n as usize + 1
                }
            })
            .map_err(Error::str)?;

        match self.0 {
            ByteArray(v) => Ok(Val(ByteArray(fastnbt::ByteArray::new(
                v.iter().take(end).skip(start).copied().collect::<Vec<_>>(),
            )))),
            IntArray(v) => Ok(Val(IntArray(fastnbt::IntArray::new(
                v.iter().take(end).skip(start).copied().collect::<Vec<_>>(),
            )))),
            LongArray(v) => Ok(Val(LongArray(fastnbt::LongArray::new(
                v.iter().take(end).skip(start).copied().collect::<Vec<_>>(),
            )))),
            List(v) => Ok(Val(List(
                v.into_iter().take(end).skip(start).collect::<Vec<_>>(),
            ))),
            _ => Err(Error::typ(self, "List")),
        }
    }

    fn map_values<'a, I: Iterator<Item = jaq_core::ValX<'a, Self>>>(
        self,
        opt: jaq_core::path::Opt,
        f: impl Fn(Self) -> I,
    ) -> jaq_core::ValX<'a, Self> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        match self.0 {
            ByteArray(v) => Ok(Val(List(
                v.iter()
                    .copied()
                    .map(Byte)
                    .map(Val)
                    .flat_map(f)
                    .map(|v| v.map(|v| v.0))
                    .collect::<Result<_, Exn<_>>>()?,
            ))),
            IntArray(v) => Ok(Val(List(
                v.iter()
                    .copied()
                    .map(Int)
                    .map(Val)
                    .flat_map(f)
                    .map(|v| v.map(|v| v.0))
                    .collect::<Result<_, Exn<_>>>()?,
            ))),
            LongArray(v) => Ok(Val(List(
                v.iter()
                    .copied()
                    .map(Long)
                    .map(Val)
                    .flat_map(f)
                    .map(|v| v.map(|v| v.0))
                    .collect::<Result<_, Exn<_>>>()?,
            ))),
            List(v) => Ok(Val(List(
                v.iter()
                    .cloned()
                    .map(Val)
                    .flat_map(f)
                    .map(|v| v.map(|v| v.0))
                    .collect::<Result<_, Exn<_>>>()?,
            ))),
            Compound(v) => {
                let mut map = HashMap::with_capacity(v.len());
                for e in v
                    .into_iter()
                    .filter_map(|(k, v)| f(Val(v)).next().map(|v| Ok::<_, Exn<_>>((k, v?))))
                {
                    let (k, v) = e?;
                    map.insert(k, v.0);
                }

                Ok(Val(Compound(map)))
            }
            v => opt.fail(Val(v), |v| Exn::from(Error::typ(v, "Iter"))),
        }
    }

    fn map_index<'a, I: Iterator<Item = jaq_core::ValX<'a, Self>>>(
        self,
        index: &Self,
        opt: jaq_core::path::Opt,
        f: impl Fn(Self) -> I,
    ) -> jaq_core::ValX<'a, Self> {
        use fastnbt::Value::*;
        use jaq_core::Error;

        match (self.0.clone(), index) {
            (ByteArray(mut v), idx) if let Ok(idx) = idx.maybe_usize() => {
                let Some(mr) = v.get_mut(idx) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(ByteArray(v)), Val(Int(idx as i32))))
                    });
                };
                let e = std::mem::take(mr);
                match f(Val(Byte(e))).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        let mut inner = v.into_inner();
                        inner.remove(idx);
                        Ok(Val(ByteArray(fastnbt::ByteArray::new(inner))))
                    }
                    Err(e) => opt.fail(self, |_| e),
                }
            }
            (IntArray(mut v), idx) if let Ok(idx) = idx.maybe_usize() => {
                let Some(mr) = v.get_mut(idx) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(IntArray(v)), Val(Int(idx as i32))))
                    });
                };
                let e = std::mem::take(mr);
                match f(Val(Int(e))).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        let mut inner = v.into_inner();
                        inner.remove(idx);
                        Ok(Val(IntArray(fastnbt::IntArray::new(inner))))
                    }
                    Err(e) => opt.fail(self, |_| e),
                }
            }
            (LongArray(mut v), idx) if let Ok(idx) = idx.maybe_usize() => {
                let Some(mr) = v.get_mut(idx) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(LongArray(v)), Val(Int(idx as i32))))
                    });
                };
                let e = std::mem::take(mr);
                match f(Val(Long(e))).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        let mut inner = v.into_inner();
                        inner.remove(idx);
                        Ok(Val(LongArray(fastnbt::LongArray::new(inner))))
                    }
                    Err(e) => opt.fail(self, |_| e),
                }
            }
            (List(mut v), idx) if let Ok(idx) = idx.maybe_usize() => {
                let Some(e) = v.get(idx) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(List(v)), Val(Int(idx as i32))))
                    });
                };
                match f(Val(e.clone())).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        v.remove(idx);
                        Ok(Val(List(v)))
                    }
                    Err(e) => opt.fail(self, |_| e),
                }
            }
            (Compound(mut map), Val(String(k))) => {
                let Some(v) = map.get(k) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(Compound(map)), Val(String(k.to_string()))))
                    });
                };

                match f(Val(v.clone())).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        map.remove(k);
                        Ok(Val(Compound(map)))
                    }
                    Err(e) => opt.fail(self, |_| e),
                }
            }
            (o, k) => opt.fail(self, |_| Exn::from(Error::index(Val(o.clone()), k.clone()))),
        }
    }

    fn map_range<'a, I: Iterator<Item = jaq_core::ValX<'a, Self>>>(
        self,
        range: jaq_core::val::Range<&Self>,
        opt: jaq_core::path::Opt,
        f: impl Fn(Self) -> I,
    ) -> jaq_core::ValX<'a, Self> {
        todo!()
    }

    fn as_bool(&self) -> bool {
        use fastnbt::Value::*;

        !matches!(self, Val(Byte(0)))
    }

    fn into_string(self) -> Self {
        Val(fastnbt::Value::String(format!("{self}")))
    }
}

impl jaq_std::ValT for Val {
    fn into_seq<S: FromIterator<Self>>(self) -> Result<S, Self> {
        use fastnbt::Value::*;

        match self.0 {
            ByteArray(a) => Ok(a.iter().copied().map(Byte).map(Val).collect()),
            IntArray(a) => Ok(a.iter().copied().map(Int).map(Val).collect()),
            LongArray(a) => Ok(a.iter().copied().map(Long).map(Val).collect()),
            List(a) => Ok(a.into_iter().map(Val).collect()),
            _ => Err(self),
        }
    }

    fn is_int(&self) -> bool {
        use fastnbt::Value::*;
        matches!(self.0, Byte(_) | Short(_) | Int(_) | Long(_))
    }

    fn as_isize(&self) -> Option<isize> {
        match self.0 {
            fastnbt::Value::Byte(n) => Some(n as isize),
            fastnbt::Value::Short(n) => Some(n as isize),
            fastnbt::Value::Int(n) => Some(n as isize),
            fastnbt::Value::Long(n) => n.try_into().ok(),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            fastnbt::Value::Byte(n) => Some(n as f64),
            fastnbt::Value::Short(n) => Some(n as f64),
            fastnbt::Value::Int(n) => Some(n as f64),
            fastnbt::Value::Long(n) => Some(n as f64),
            fastnbt::Value::Float(n) => Some(n as f64),
            fastnbt::Value::Double(n) => Some(n),
            _ => None,
        }
    }

    fn is_utf8_str(&self) -> bool {
        matches!(self.0, fastnbt::Value::String(_))
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            fastnbt::Value::String(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    fn as_sub_str(&self, sub: &[u8]) -> Self {
        if let Some(s) = self.0.as_str()
            && let Some(range) = s.as_bytes().subslice_range(sub)
        {
            Val(fastnbt::Value::String(s[range].to_string()))
        } else {
            self.clone()
        }
    }

    fn from_utf8_bytes(b: impl AsRef<[u8]> + Send + 'static) -> Self {
        Val(fastnbt::Value::String(
            std::str::from_utf8(b.as_ref()).unwrap().to_string(),
        ))
    }
}
