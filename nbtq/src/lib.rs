pub mod print;

use std::{cmp::Ordering, fmt::Debug};

pub use anyhow::Error;
use anyhow::anyhow;
use bytes::{BufMut, Bytes, BytesMut};
pub use crab_nbt as nbt;
use crab_nbt::{NbtCompound, NbtList, NbtTag};
use jaq_core::{
    DataT, Exn, RunPtr,
    box_iter::box_once,
    load,
    native::{Filter, Fun, bome, v},
    ops,
};

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq)]
pub struct Val(pub NbtTag);

impl Val {
    pub fn len(&self) -> Option<usize> {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len().is_some_and(|len| len == 0)
    }

    fn range_int(
        range: jaq_core::val::Range<&Self>,
    ) -> Result<jaq_core::val::Range<usize>, jaq_core::Error<Val>> {
        let f = |i: Option<&Self>| i.as_ref().map(|i| i.maybe_usize()).transpose();
        Ok(f(range.start)?..f(range.end)?)
    }
}

pub trait NbtExtension {
    type Error;

    fn maybe_i64(&self) -> Result<i64, Self::Error>;

    fn maybe_usize(&self) -> Result<usize, Self::Error>;

    fn len(&self) -> Option<usize>;

    fn is_empty(&self) -> bool {
        self.len().is_some_and(|len| len == 0)
    }
}

impl NbtExtension for NbtTag {
    type Error = crate::Error;

    fn maybe_i64(&self) -> Result<i64, Self::Error> {
        match self {
            NbtTag::Byte(v) => Ok(*v as i64),
            NbtTag::Short(v) => Ok(*v as i64),
            NbtTag::Int(v) => Ok(*v as i64),
            NbtTag::Long(v) => Ok(*v),
            NbtTag::Float(v) => {
                if *v == (*v as i64) as f32 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f32 conversion would be lossy"))
                }
            }
            NbtTag::Double(v) => {
                if *v == (*v as i64) as f64 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f64 conversion would be lossy"))
                }
            }
            _ => Err(anyhow!("not a numeric type")),
        }
    }

    fn maybe_usize(&self) -> Result<usize, Self::Error> {
        match self {
            NbtTag::Byte(v) => Ok(*v as usize),
            NbtTag::Short(v) => Ok(*v as usize),
            NbtTag::Int(v) => Ok(*v as usize),
            NbtTag::Long(v) => Ok((*v).try_into()?),
            NbtTag::Float(v) => {
                if *v == (*v as usize) as f32 {
                    Ok(*v as usize)
                } else {
                    Err(anyhow!("f32 conversion would be lossy"))
                }
            }
            NbtTag::Double(v) => {
                if *v == (*v as usize) as f64 {
                    Ok(*v as usize)
                } else {
                    Err(anyhow!("f64 conversion would be lossy"))
                }
            }
            _ => Err(anyhow!("not a numeric type")),
        }
    }

    fn len(&self) -> Option<usize> {
        Some(match self {
            NbtTag::ByteArray(v) => v.len(),
            NbtTag::String(v) => v.len(),
            NbtTag::List(v) => v.len(),
            NbtTag::Compound(v) => v.child_tags.len(),
            NbtTag::IntArray(v) => v.len(),
            NbtTag::LongArray(v) => v.len(),
            _ => return None,
        })
    }
}

impl NbtExtension for Val {
    type Error = jaq_core::Error<Val>;

    fn maybe_i64(&self) -> Result<i64, Self::Error> {
        self.0
            .maybe_i64()
            .map_err(|_| jaq_core::Error::typ(self.clone(), "Num"))
    }

    fn maybe_usize(&self) -> Result<usize, Self::Error> {
        self.0
            .maybe_usize()
            .map_err(|_| jaq_core::Error::typ(self.clone(), "Num"))
    }

    fn len(&self) -> Option<usize> {
        self.0.len()
    }
}

pub type ValR = jaq_core::ValR<Val>;
pub type ValX<'a> = jaq_core::ValX<'a, Val>;

// this is incorrect, but necessary for implementing jaq_std::ValT
impl Eq for Val {}

// this is incorrect, but necessary for implementing jaq_core::ValT
impl Ord for Val {
    fn cmp(&self, other: &Self) -> Ordering {
        use NbtTag::*;

        let tag = self.0.get_type_id().cmp(&other.0.get_type_id());
        if tag != Ordering::Equal {
            return tag;
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

impl std::fmt::Display for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        print::write_snbt_string(f, &self.0, Default::default())?;
        Ok(())
    }
}

impl From<bool> for Val {
    fn from(value: bool) -> Self {
        Val(NbtTag::Byte(if value { 1 } else { 0 }))
    }
}

impl From<jaq_core::val::Range<Self>> for Val {
    fn from(value: jaq_core::val::Range<Self>) -> Self {
        let mut map = NbtCompound::new();
        if let Some(v) = value.start {
            map.put("start".to_string(), v.0)
        }
        if let Some(v) = value.end {
            map.put("end".to_string(), v.0)
        }
        Val(NbtTag::Compound(map))
    }
}

impl From<isize> for Val {
    fn from(value: isize) -> Self {
        Val(NbtTag::Int(value as i32))
    }
}

impl From<usize> for Val {
    fn from(value: usize) -> Self {
        if value > isize::MAX as usize {
            Val(NbtTag::Long(value as i64))
        } else {
            Val(NbtTag::Int(value as i32))
        }
    }
}

impl From<f64> for Val {
    fn from(value: f64) -> Self {
        Val(NbtTag::Double(value))
    }
}

impl From<String> for Val {
    fn from(value: String) -> Self {
        Val(NbtTag::String(value))
    }
}

impl FromIterator<Self> for Val {
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut list = NbtList::with_capacity(iter.size_hint().0);
        for value in iter {
            list.push(value.0);
        }
        Val(NbtTag::List(list))
    }
}

impl core::ops::Add for Val {
    type Output = ValR;

    fn add(self, rhs: Self) -> Self::Output {
        use NbtTag::{
            Byte, ByteArray, Compound, Double, Float, Int, IntArray, List, Long, LongArray, Short,
            String as Str,
        };
        use jaq_core::Error;

        fn concatl<T: Into<NbtTag>>(mut a: NbtList, b: impl IntoIterator<Item = T>) -> NbtTag {
            a.extend(b);
            List(a)
        }

        fn concatr<T: Into<NbtTag>>(a: impl IntoIterator<Item = T>, b: NbtList) -> NbtTag {
            let iter = a.into_iter();
            let mut list = NbtList::with_capacity(iter.size_hint().0 + b.len());
            list.extend(iter);
            list.extend(b);
            List(list)
        }

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
            (List(a), List(b)) => concatl(a, b),
            (List(a), ByteArray(b)) => {
                concatl(a, b.iter().copied().map(|b| NbtTag::Byte(b.cast_signed())))
            }
            (List(a), IntArray(b)) => concatl(a, b),
            (List(a), LongArray(b)) => concatl(a, b),
            (ByteArray(a), List(b)) => {
                concatr(a.iter().copied().map(|b| NbtTag::Byte(b.cast_signed())), b)
            }
            (IntArray(a), List(b)) => concatr(a, b),
            (LongArray(a), List(b)) => concatr(a, b),
            (Compound(a), Compound(b)) => {
                let mut map = a.clone();
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
        use NbtTag::*;
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
            (List(mut a), List(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(e));
                }
                List(a)
            }
            (List(mut a), ByteArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !matches!(e, NbtTag::Byte(e) if b.contains(&e.cast_unsigned())));
                }
                List(a)
            }
            (List(mut a), IntArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !matches!(e, NbtTag::Int(e) if b.contains(e)));
                }
                List(a)
            }
            (List(mut a), LongArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !matches!(e, NbtTag::Long(e) if b.contains(e)));
                }
                List(a)
            }
            (ByteArray(a), List(b)) => {
                if !b.is_empty() {
                    let mut bm = BytesMut::new();
                    for e in a {
                        if b.contains(&NbtTag::Byte(e.cast_signed())) {
                            bm.put_i8(e.cast_signed());
                        }
                    }
                    ByteArray(bm.into())
                } else {
                    ByteArray(a)
                }
            }
            (IntArray(mut a), List(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(&NbtTag::Int(*e)));
                }
                IntArray(a)
            }
            (LongArray(mut a), List(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(&NbtTag::Long(*e)));
                }
                LongArray(a)
            }
            (ByteArray(a), ByteArray(b)) => {
                if !b.is_empty() {
                    let mut bm = BytesMut::new();
                    for e in a {
                        if b.contains(&e) {
                            bm.put_i8(e.cast_signed());
                        }
                    }
                    ByteArray(bm.into())
                } else {
                    ByteArray(a)
                }
            }
            (ByteArray(a), IntArray(b)) => {
                if !b.is_empty() {
                    let mut bm = BytesMut::new();
                    for e in a {
                        if b.contains(&(e as i32)) {
                            bm.put_i8(e.cast_signed());
                        }
                    }
                    ByteArray(bm.into())
                } else {
                    ByteArray(a)
                }
            }
            (ByteArray(a), LongArray(b)) => {
                if !b.is_empty() {
                    let mut bm = BytesMut::new();
                    for e in a {
                        if b.contains(&(e as i64)) {
                            bm.put_i8(e.cast_signed());
                        }
                    }
                    ByteArray(bm.into())
                } else {
                    ByteArray(a)
                }
            }
            (IntArray(mut a), ByteArray(b)) => {
                if !b.is_empty() {
                    a.retain(|&e| e.try_into().map(|e| !b.contains(&e)).unwrap_or(true));
                }
                IntArray(a)
            }
            (IntArray(mut a), IntArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(e));
                }
                IntArray(a)
            }
            (IntArray(mut a), LongArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(&(*e as i64)));
                }
                IntArray(a)
            }
            (LongArray(mut a), ByteArray(b)) => {
                if !b.is_empty() {
                    a.retain(|&e| e.try_into().map(|e| !b.contains(&e)).unwrap_or(true));
                }
                LongArray(a)
            }
            (LongArray(mut a), IntArray(b)) => {
                if !b.is_empty() {
                    a.retain(|&e| e.try_into().map(|e| !b.contains(&e)).unwrap_or(true));
                }
                LongArray(a)
            }
            (LongArray(mut a), LongArray(b)) => {
                if !b.is_empty() {
                    a.retain(|e| !b.contains(e));
                }
                LongArray(a)
            }
            (l, r) => return Err(Error::math(Val(l), ops::Math::Sub, Val(r))),
        }))
    }
}

impl core::ops::Mul for Val {
    type Output = ValR;
    fn mul(self, rhs: Self) -> Self::Output {
        use NbtTag::{Byte, Compound, Double, Float, Int, Long, Short, String as Str};
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
        use NbtTag::{Byte, Double, Float, Int, Long, Short};
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
        use NbtTag::{Byte, Double, Float, Int, Long, Short};
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
        use NbtTag::{Byte, Double, Float, Int, Long, Short};
        use jaq_core::Error;

        Ok(Val(match self.0 {
            Byte(n) => Byte(-n),
            Short(n) => Short(-n),
            Int(n) => Int(-n),
            Long(n) => Long(-n),
            Float(n) => Float(-n),
            Double(n) => Double(-n),
            n => return Err(Error::typ(Val(n), "Num")),
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
    use NbtTag::*;
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
        use NbtTag::{Byte, Double, Int, Long, Short, String};

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
        use NbtTag::{Compound, String};
        use jaq_core::Error;

        let mut map = NbtCompound::new();
        for (k, v) in iter {
            let String(k) = k.0 else {
                return Err(Error::typ(k, "String"));
            };
            map.put(k, v.0);
        }

        Ok(Val(Compound(map)))
    }

    fn key_values(
        self,
    ) -> jaq_core::box_iter::BoxIter<'static, jaq_core::ValR<(Self, Self), Self>> {
        use NbtTag::*;
        use jaq_core::Error;

        match self.0 {
            ByteArray(v) => Box::new(
                v.iter()
                    .copied()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(Byte(n.cast_signed())))))
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
                v.into_iter()
                    .enumerate()
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(n))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            Compound(v) => Box::new(
                v.into_iter()
                    .map(|(k, v)| Ok((Val(String(k.to_string())), Val(v.clone()))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            _ => box_once(Err(Error::typ(self, "Iter"))),
        }
    }

    fn values(self) -> Box<dyn Iterator<Item = jaq_core::ValR<Self>>> {
        use NbtTag::*;
        use jaq_core::Error;

        match self.0 {
            ByteArray(v) => Box::new(
                v.into_iter()
                    .map(u8::cast_signed)
                    .map(Byte)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            IntArray(v) => Box::new(
                v.into_iter()
                    .map(Int)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            LongArray(v) => Box::new(
                v.into_iter()
                    .map(Long)
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            List(v) => Box::new(
                v.into_iter()
                    .map(Val)
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            Compound(v) => Box::new(
                v.into_iter()
                    .map(|(_, v)| Val(v))
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            ),
            _ => box_once(Err(Error::typ(self, "Iter"))),
        }
    }

    fn index(self, index: &Self) -> jaq_core::ValR<Self> {
        use NbtTag::*;
        use jaq_core::Error;

        match (self.0, index) {
            (ByteArray(v), idx) if let Ok(idx) = idx.maybe_usize() => v
                .get(idx)
                .copied()
                .map(u8::cast_signed)
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
                .map(|e| Val(e.clone()))
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
        use NbtTag::*;
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

        Ok(Val(match self.0 {
            ByteArray(v) => ByteArray(v.iter().take(end).skip(start).copied().collect::<Bytes>()),
            IntArray(v) => IntArray(v.iter().take(end).skip(start).copied().collect::<Vec<_>>()),
            LongArray(v) => LongArray(v.iter().take(end).skip(start).copied().collect::<Vec<_>>()),
            List(v) => {
                let list = NbtList::from_iter(v.into_iter().take(end).skip(start));
                List(list)
            }
            _ => return Err(Error::typ(self, "List")),
        }))
    }

    fn map_values<'a, I: Iterator<Item = jaq_core::ValX<'a, Self>>>(
        self,
        opt: jaq_core::path::Opt,
        f: impl Fn(Self) -> I,
    ) -> jaq_core::ValX<'a, Self> {
        use NbtTag::*;
        use jaq_core::Error;

        Ok(Val(match self.0 {
            ByteArray(v) => {
                let mut list = NbtList::new();
                for value in v
                    .iter()
                    .copied()
                    .map(u8::cast_signed)
                    .map(Byte)
                    .map(Val)
                    .flat_map(f)
                {
                    match value {
                        Ok(value) => {
                            list.push(value.0);
                        }
                        Err(e) => {
                            return opt.fail(Val(ByteArray(v)), |_| e);
                        }
                    }
                }
                NbtTag::List(list)
            }
            IntArray(v) => {
                let mut list = NbtList::new();
                for value in v.iter().copied().map(Int).map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            list.push(value.0);
                        }
                        Err(e) => {
                            return opt.fail(Val(IntArray(v)), |_| e);
                        }
                    }
                }
                NbtTag::List(list)
            }
            LongArray(v) => {
                let mut list = NbtList::new();
                for value in v.iter().copied().map(Long).map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            list.push(value.0);
                        }
                        Err(e) => {
                            return opt.fail(Val(LongArray(v)), |_| e);
                        }
                    }
                }
                NbtTag::List(list)
            }
            List(v) => {
                let mut list = NbtList::new();
                for value in v.iter().cloned().map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            list.push(value.0);
                        }
                        Err(e) => {
                            return opt.fail(Val(List(v)), |_| e);
                        }
                    }
                }
                NbtTag::List(list)
            }
            Compound(v) => {
                let mut map = NbtCompound::new();
                for e in v
                    .into_iter()
                    .filter_map(|(k, v)| f(Val(v)).next().map(|v| Ok::<_, Exn<_>>((k, v?))))
                {
                    let (k, v) = e?;
                    map.put(k, v.0);
                }

                Compound(map)
            }
            v => return opt.fail(Val(v), |v| Exn::from(Error::typ(v, "Iter"))),
        }))
    }

    fn map_index<'a, I: Iterator<Item = jaq_core::ValX<'a, Self>>>(
        self,
        index: &Self,
        opt: jaq_core::path::Opt,
        f: impl Fn(Self) -> I,
    ) -> jaq_core::ValX<'a, Self> {
        use NbtTag::*;
        use jaq_core::Error;

        match (self.0.clone(), index) {
            (ByteArray(v), idx) if let Ok(idx) = idx.maybe_usize() => {
                let Some(e) = v.get(idx).copied().map(u8::cast_signed) else {
                    return opt.fail(self, |_| {
                        Exn::from(Error::index(Val(ByteArray(v)), Val(Int(idx as i32))))
                    });
                };
                match f(Val(Byte(e))).next().transpose() {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => {
                        let mut bm = v.to_vec();
                        bm.remove(idx);
                        Ok(Val(ByteArray(Bytes::from(bm))))
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
                        v.remove(idx);
                        Ok(Val(IntArray(v)))
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
                        v.remove(idx);
                        Ok(Val(LongArray(v)))
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
                        map.child_tags.retain(|(ok, _)| k != ok);
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
        let (start, end) = match Self::range_int(range) {
            Ok(range) => (range.start, range.end),
            Err(e) => return opt.fail(self, |_| Exn::from(e)),
        };

        match self.0 {
            NbtTag::String(s) => {
                let s = match (start, end) {
                    (None, None) => s,
                    (None, Some(end)) => s[..end].to_string(),
                    (Some(start), None) => s[start..].to_string(),
                    (Some(start), Some(end)) => s[start..end].to_string(),
                };
                let y = f(Val(NbtTag::String(s))).next();
                Ok(y.transpose()?
                    .unwrap_or_else(|| Val(NbtTag::String(String::new()))))
            }
            NbtTag::ByteArray(v) => {
                let v = match (start, end) {
                    (None, None) => v.to_vec(),
                    (None, Some(end)) => v[..end].to_vec(),
                    (Some(start), None) => v[start..].to_vec(),
                    (Some(start), Some(end)) => v[start..end].to_vec(),
                };
                let y = f(Val(NbtTag::ByteArray(v.into()))).next();
                Ok(y.transpose()?
                    .unwrap_or_else(|| Val(NbtTag::ByteArray(Bytes::new()))))
            }
            NbtTag::IntArray(v) => {
                let v = match (start, end) {
                    (None, None) => v,
                    (None, Some(end)) => v[..end].to_vec(),
                    (Some(start), None) => v[start..].to_vec(),
                    (Some(start), Some(end)) => v[start..end].to_vec(),
                };
                let y = f(Val(NbtTag::IntArray(v))).next();
                Ok(y.transpose()?
                    .unwrap_or_else(|| Val(NbtTag::IntArray(Vec::new()))))
            }
            NbtTag::LongArray(v) => {
                let v = match (start, end) {
                    (None, None) => v,
                    (None, Some(end)) => v[..end].to_vec(),
                    (Some(start), None) => v[start..].to_vec(),
                    (Some(start), Some(end)) => v[start..end].to_vec(),
                };
                let y = f(Val(NbtTag::LongArray(v))).next();
                Ok(y.transpose()?
                    .unwrap_or_else(|| Val(NbtTag::LongArray(Vec::new()))))
            }
            NbtTag::List(v) => {
                let v = match (start, end) {
                    (None, None) => v,
                    (None, Some(end)) => {
                        let mut vec = NbtList::new();
                        vec.extend(v.into_iter().take(end));
                        vec
                    }
                    (Some(start), None) => {
                        let mut vec = NbtList::new();
                        vec.extend(v.into_iter().skip(start));
                        vec
                    }
                    (Some(start), Some(end)) => {
                        let mut vec = NbtList::new();
                        vec.extend(v.into_iter().take(end).skip(start));
                        vec
                    }
                };
                let y = f(Val(NbtTag::List(v))).next();
                Ok(y.transpose()?.unwrap_or(Val(NbtTag::List(NbtList::new()))))
            }
            _ => opt.fail(self, |v| Exn::from(jaq_core::Error::typ(v, "List"))),
        }
    }

    fn as_bool(&self) -> bool {
        use NbtTag::*;

        !matches!(self, Val(Byte(0)))
    }

    fn into_string(self) -> Self {
        Val(NbtTag::String(format!("{self}")))
    }
}

impl jaq_std::ValT for Val {
    fn into_seq<S: FromIterator<Self>>(self) -> Result<S, Self> {
        use NbtTag::*;

        match self.0 {
            ByteArray(a) => Ok(a
                .iter()
                .copied()
                .map(u8::cast_signed)
                .map(Byte)
                .map(Val)
                .collect()),
            IntArray(a) => Ok(a.iter().copied().map(Int).map(Val).collect()),
            LongArray(a) => Ok(a.iter().copied().map(Long).map(Val).collect()),
            List(a) => Ok(a.into_iter().map(Val).collect()),
            _ => Err(self),
        }
    }

    fn is_int(&self) -> bool {
        use NbtTag::*;
        matches!(self.0, Byte(_) | Short(_) | Int(_) | Long(_))
    }

    fn as_isize(&self) -> Option<isize> {
        match self.0 {
            NbtTag::Byte(n) => Some(n as isize),
            NbtTag::Short(n) => Some(n as isize),
            NbtTag::Int(n) => Some(n as isize),
            NbtTag::Long(n) => n.try_into().ok(),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            NbtTag::Byte(n) => Some(n as f64),
            NbtTag::Short(n) => Some(n as f64),
            NbtTag::Int(n) => Some(n as f64),
            NbtTag::Long(n) => Some(n as f64),
            NbtTag::Float(n) => Some(n as f64),
            NbtTag::Double(n) => Some(n),
            _ => None,
        }
    }

    fn is_utf8_str(&self) -> bool {
        matches!(self.0, NbtTag::String(_))
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            NbtTag::String(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    fn as_sub_str(&self, sub: &[u8]) -> Self {
        if !sub.is_empty()
            && let NbtTag::String(s) = &self.0
        {
            let bytes = s.as_bytes();
            for window in bytes.windows(sub.len()) {
                if window == sub {
                    return Val(NbtTag::String(
                        std::str::from_utf8(bytes).unwrap().to_string(),
                    ));
                }
            }
        }

        panic!()
    }

    fn from_utf8_bytes(b: impl AsRef<[u8]> + Send + 'static) -> Self {
        Val(NbtTag::String(
            std::str::from_utf8(b.as_ref()).unwrap().to_string(),
        ))
    }
}
