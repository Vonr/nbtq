pub mod print;

use std::{cmp::Ordering, collections::HashSet, fmt::Debug};

pub use anyhow::Error;
use anyhow::anyhow;
use jaq_core::{
    DataT, Exn, RunPtr,
    box_iter::box_once,
    load,
    native::{Filter, Fun, bome, v},
    ops,
};
use valence_nbt::List as VList;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq)]
pub struct Val(pub valence_nbt::Value);

impl Val {
    pub fn len(&self) -> Option<usize> {
        match &self.0 {
            valence_nbt::Value::String(s) => Some(s.len()),
            valence_nbt::Value::ByteArray(s) => Some(s.len()),
            valence_nbt::Value::IntArray(s) => Some(s.len()),
            valence_nbt::Value::LongArray(s) => Some(s.len()),
            valence_nbt::Value::List(s) => Some(s.len()),
            valence_nbt::Value::Compound(s) => Some(s.len()),
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

    fn len(&self) -> Option<usize>;

    fn is_empty(&self) -> bool {
        self.len().is_some_and(|len| len == 0)
    }
}

impl NbtExtension for valence_nbt::Value {
    fn maybe_i64(&self) -> Result<i64> {
        match self {
            valence_nbt::Value::Byte(v) => Ok(*v as i64),
            valence_nbt::Value::Short(v) => Ok(*v as i64),
            valence_nbt::Value::Int(v) => Ok(*v as i64),
            valence_nbt::Value::Long(v) => Ok(*v),
            valence_nbt::Value::Float(v) => {
                if *v == (*v as i64) as f32 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f32 conversion would be lossy"))
                }
            }
            valence_nbt::Value::Double(v) => {
                if *v == (*v as i64) as f64 {
                    Ok(*v as i64)
                } else {
                    Err(anyhow!("f64 conversion would be lossy"))
                }
            }
            _ => Err(anyhow!("not a numeric type")),
        }
    }

    fn len(&self) -> Option<usize> {
        Some(match self {
            valence_nbt::Value::ByteArray(v) => v.len(),
            valence_nbt::Value::String(v) => v.len(),
            valence_nbt::Value::List(v) => v.len(),
            valence_nbt::Value::Compound(v) => v.len(),
            valence_nbt::Value::IntArray(v) => v.len(),
            valence_nbt::Value::LongArray(v) => v.len(),
            _ => return None,
        })
    }
}

impl NbtExtension for Val {
    fn maybe_i64(&self) -> Result<i64> {
        self.0.maybe_i64()
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
        use valence_nbt::Value::*;

        let tag = self.0.tag().cmp(&other.0.tag());
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
        Val(valence_nbt::Value::Byte(if value { 1 } else { 0 }))
    }
}

impl From<jaq_core::val::Range<Self>> for Val {
    fn from(value: jaq_core::val::Range<Self>) -> Self {
        let mut map = valence_nbt::Compound::with_capacity(2);
        value
            .start
            .and_then(|v| map.insert("start".to_string(), v.0));
        value.end.and_then(|v| map.insert("end".to_string(), v.0));
        Val(valence_nbt::Value::Compound(map))
    }
}

impl From<isize> for Val {
    fn from(value: isize) -> Self {
        Val(valence_nbt::Value::Int(value as i32))
    }
}

impl From<usize> for Val {
    fn from(value: usize) -> Self {
        if value > isize::MAX as usize {
            Val(valence_nbt::Value::Long(value as i64))
        } else {
            Val(valence_nbt::Value::Int(value as i32))
        }
    }
}

impl From<f64> for Val {
    fn from(value: f64) -> Self {
        Val(valence_nbt::Value::Double(value))
    }
}

impl From<String> for Val {
    fn from(value: String) -> Self {
        Val(valence_nbt::Value::String(value))
    }
}

impl FromIterator<Self> for Val {
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let mut list = VList::new();
        for value in iter {
            let ty = value.0.tag();

            if !list.try_push(value.0) {
                panic!(
                    "tried to insert {ty:?} into list of type {:?}",
                    list.element_tag()
                )
            }
        }
        Val(valence_nbt::Value::List(list))
    }
}

impl core::ops::Add for Val {
    type Output = ValR;

    fn add(self, rhs: Self) -> Self::Output {
        use jaq_core::Error;
        use valence_nbt::Value::{
            Byte, ByteArray, Compound, Double, Float, Int, IntArray, List, Long, LongArray, Short,
            String as Str,
        };

        fn concatl<A: Clone + From<B>, B: Clone>(mut a: Vec<A>, b: &[B]) -> Vec<A> {
            a.reserve(b.len());
            a.extend(b.iter().map(|e| A::from(e.clone())));
            a
        }

        fn concatr<A: Clone, B: Clone + From<A>>(a: &[A], b: &[B]) -> Vec<B> {
            let mut v = Vec::with_capacity(a.len() + b.len());
            v.extend(a.iter().map(|e| B::from(e.clone())));
            v.extend_from_slice(b);
            v
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
            (List(a), List(b)) if a.element_tag() == b.element_tag() => List(match (a, b) {
                (VList::End, VList::End) => VList::End,
                (VList::Byte(a), VList::Byte(b)) => concatl(a, &b).into(),
                (VList::Short(a), VList::Short(b)) => concatl(a, &b).into(),
                (VList::Int(a), VList::Int(b)) => concatl(a, &b).into(),
                (VList::Long(a), VList::Long(b)) => concatl(a, &b).into(),
                (VList::Float(a), VList::Float(b)) => concatl(a, &b).into(),
                (VList::Double(a), VList::Double(b)) => concatl(a, &b).into(),
                (VList::ByteArray(a), VList::ByteArray(b)) => concatl(a, &b).into(),
                (VList::String(a), VList::String(b)) => concatl(a, &b).into(),
                (VList::List(a), VList::List(b)) => concatl(a, &b).into(),
                (VList::Compound(a), VList::Compound(b)) => concatl(a, &b).into(),
                (VList::IntArray(a), VList::IntArray(b)) => concatl(a, &b).into(),
                (VList::LongArray(a), VList::LongArray(b)) => concatl(a, &b).into(),
                _ => unreachable!(),
            }),
            (a @ ByteArray(_), List(VList::End)) => a,
            (a @ IntArray(_), List(VList::End)) => a,
            (a @ LongArray(_), List(VList::End)) => a,
            (a @ List(_), List(VList::End)) => a,
            (List(VList::End), b @ ByteArray(_)) => b,
            (List(VList::End), b @ IntArray(_)) => b,
            (List(VList::End), b @ LongArray(_)) => b,
            (List(VList::End), b @ List(_)) => b,
            (List(VList::Byte(mut a)), ByteArray(b))
            | (List(VList::Byte(mut a)), List(VList::Byte(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (List(VList::Byte(a)), IntArray(b)) => concatr(&a, &b).into(),
            (List(VList::Byte(a)), List(VList::Int(b))) => concatr(&a, &b).into(),
            (List(VList::Byte(a)), LongArray(b)) => concatr(&a, &b).into(),
            (List(VList::Byte(a)), List(VList::Long(b))) => concatr(&a, &b).into(),
            (ByteArray(mut a), ByteArray(b)) | (ByteArray(mut a), List(VList::Byte(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (ByteArray(a), IntArray(b)) => concatr(&a, &b).into(),
            (ByteArray(a), LongArray(b)) => concatr(&a, &b).into(),
            (ByteArray(a), List(VList::Int(b))) => concatr(&a, &b).into(),
            (ByteArray(a), List(VList::Long(b))) => concatr(&a, &b).into(),
            (List(VList::Int(mut a)), IntArray(b))
            | (List(VList::Int(mut a)), List(VList::Int(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (List(VList::Int(a)), ByteArray(b)) => concatl(a, &b).into(),
            (List(VList::Int(a)), List(VList::Byte(b))) => concatl(a, &b).into(),
            (List(VList::Int(a)), LongArray(b)) => concatr(&a, &b).into(),
            (List(VList::Int(a)), List(VList::Long(b))) => concatr(&a, &b).into(),
            (IntArray(mut a), IntArray(b)) | (IntArray(mut a), List(VList::Int(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (IntArray(a), ByteArray(b)) => concatl(a, &b).into(),
            (IntArray(a), LongArray(b)) => concatr(&a, &b).into(),
            (IntArray(a), List(VList::Byte(b))) => concatl(a, &b).into(),
            (IntArray(a), List(VList::Long(b))) => concatr(&a, &b).into(),
            (List(VList::Long(mut a)), LongArray(b))
            | (List(VList::Long(mut a)), List(VList::Long(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (List(VList::Long(a)), ByteArray(b)) => concatl(a, &b).into(),
            (List(VList::Long(a)), List(VList::Byte(b))) => concatl(a, &b).into(),
            (List(VList::Long(a)), IntArray(b)) => concatl(a, &b).into(),
            (List(VList::Long(a)), List(VList::Int(b))) => concatl(a, &b).into(),
            (LongArray(mut a), LongArray(b)) | (LongArray(mut a), List(VList::Long(b))) => {
                a.extend_from_slice(&b);
                a.into()
            }
            (LongArray(a), ByteArray(b)) => concatl(a, &b).into(),
            (LongArray(a), IntArray(b)) => concatl(a, &b).into(),
            (LongArray(a), List(VList::Byte(b))) => concatl(a, &b).into(),
            (LongArray(a), List(VList::Int(b))) => concatl(a, &b).into(),
            // TODO:Generic list
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
        use jaq_core::Error;
        use valence_nbt::Value::{
            Byte, ByteArray, Double, Float, Int, IntArray, List, Long, LongArray, Short,
        };

        fn list<A, B>(a: &[A], b: &[B]) -> Vec<A>
        where
            A: Clone + Eq + std::hash::Hash + TryFrom<B>,
            B: Clone,
        {
            if b.len() > 32 {
                let set: HashSet<A> =
                    HashSet::from_iter(b.iter().filter_map(|e| e.clone().try_into().ok()));
                a.iter().filter(|n| !set.contains(n)).cloned().collect()
            } else {
                partial_list(a, b)
            }
        }

        fn partial_list<A, B>(a: &[A], b: &[B]) -> Vec<A>
        where
            A: Clone + PartialEq + TryFrom<B>,
            B: Clone,
        {
            let b: Box<[A]> = b.iter().filter_map(|e| e.clone().try_into().ok()).collect();
            a.iter().filter(|n| !b.contains(n)).cloned().collect()
        }

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
            (List(a), List(b)) if a.element_tag() == b.element_tag() => List(match (a, b) {
                (VList::End, VList::End) => VList::End,
                (VList::Byte(a), VList::Byte(b)) => list(&a, &b).into(),
                (VList::Short(a), VList::Short(b)) => list(&a, &b).into(),
                (VList::Int(a), VList::Int(b)) => list(&a, &b).into(),
                (VList::Long(a), VList::Long(b)) => list(&a, &b).into(),
                (VList::Float(a), VList::Float(b)) => partial_list(&a, &b).into(),
                (VList::Double(a), VList::Double(b)) => partial_list(&a, &b).into(),
                (VList::ByteArray(a), VList::ByteArray(b)) => list(&a, &b).into(),
                (VList::String(a), VList::String(b)) => list(&a, &b).into(),
                (VList::List(a), VList::List(b)) => partial_list(&a, &b).into(),
                (VList::Compound(a), VList::Compound(b)) => partial_list(&a, &b).into(),
                (VList::IntArray(a), VList::IntArray(b)) => list(&a, &b).into(),
                (VList::LongArray(a), VList::LongArray(b)) => list(&a, &b).into(),
                _ => unreachable!(),
            }),
            (a @ List(VList::End), ByteArray(_)) => a,
            (a @ List(VList::End), IntArray(_)) => a,
            (a @ List(VList::End), LongArray(_)) => a,
            (a @ List(VList::End), List(_)) => a,
            (a @ ByteArray(_), List(VList::End)) => a,
            (a @ IntArray(_), List(VList::End)) => a,
            (a @ LongArray(_), List(VList::End)) => a,
            (a @ List(_), List(VList::End)) => a,
            (ByteArray(a), ByteArray(b))
            | (ByteArray(a), List(VList::Byte(b)))
            | (List(VList::Byte(a)), ByteArray(b))
            | (List(VList::Byte(a)), List(VList::Byte(b))) => list(&a, &b).into(),
            (ByteArray(a), IntArray(b))
            | (ByteArray(a), List(VList::Int(b)))
            | (List(VList::Byte(a)), IntArray(b))
            | (List(VList::Byte(a)), List(VList::Int(b))) => list(&a, &b).into(),
            (ByteArray(a), LongArray(b))
            | (ByteArray(a), List(VList::Long(b)))
            | (List(VList::Byte(a)), LongArray(b))
            | (List(VList::Byte(a)), List(VList::Long(b))) => list(&a, &b).into(),
            (IntArray(a), ByteArray(b))
            | (IntArray(a), List(VList::Byte(b)))
            | (List(VList::Int(a)), ByteArray(b))
            | (List(VList::Int(a)), List(VList::Byte(b))) => list(&a, &b).into(),
            (IntArray(a), IntArray(b))
            | (IntArray(a), List(VList::Int(b)))
            | (List(VList::Int(a)), IntArray(b))
            | (List(VList::Int(a)), List(VList::Int(b))) => list(&a, &b).into(),
            (IntArray(a), LongArray(b))
            | (IntArray(a), List(VList::Long(b)))
            | (List(VList::Int(a)), LongArray(b))
            | (List(VList::Int(a)), List(VList::Long(b))) => list(&a, &b).into(),
            (LongArray(a), ByteArray(b))
            | (LongArray(a), List(VList::Byte(b)))
            | (List(VList::Long(a)), ByteArray(b))
            | (List(VList::Long(a)), List(VList::Byte(b))) => list(&a, &b).into(),
            (LongArray(a), IntArray(b))
            | (LongArray(a), List(VList::Int(b)))
            | (List(VList::Long(a)), IntArray(b))
            | (List(VList::Long(a)), List(VList::Int(b))) => list(&a, &b).into(),
            (LongArray(a), LongArray(b))
            | (LongArray(a), List(VList::Long(b)))
            | (List(VList::Long(a)), LongArray(b))
            | (List(VList::Long(a)), List(VList::Long(b))) => list(&a, &b).into(),
            (l, r) => return Err(Error::math(Val(l), ops::Math::Sub, Val(r))),
        }))
    }
}

impl core::ops::Mul for Val {
    type Output = ValR;
    fn mul(self, rhs: Self) -> Self::Output {
        use jaq_core::Error;
        use valence_nbt::Value::{Byte, Compound, Double, Float, Int, Long, Short, String as Str};

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
        use jaq_core::Error;
        use valence_nbt::Value::{Byte, Double, Float, Int, Long, Short};

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
        use jaq_core::Error;
        use valence_nbt::Value::{Byte, Double, Float, Int, Long, Short};

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
        use jaq_core::Error;
        use valence_nbt::Value::{Byte, Double, Float, Int, Long, Short};

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
    use jaq_core::Error;
    use valence_nbt::Value::*;
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
        use valence_nbt::Value::{Byte, Double, Int, Long, Short, String};

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
        use jaq_core::Error;
        use valence_nbt::Value::{Compound, String};

        let mut map = valence_nbt::Compound::new();
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
        use jaq_core::Error;
        use valence_nbt::Value::*;

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
                    .map(|(i, n)| Ok((Val(Int(i.try_into().unwrap())), Val(n.into()))))
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
        use jaq_core::Error;
        use valence_nbt::Value::*;

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
                    .map(|e| Val(e.into()))
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
        use jaq_core::Error;
        use valence_nbt::Value::*;

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
                .map(|e| Val(e.into()))
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
        use jaq_core::Error;
        use valence_nbt::Value::*;

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
            ByteArray(v) => ByteArray(v.iter().take(end).skip(start).copied().collect::<Vec<_>>()),
            IntArray(v) => IntArray(v.iter().take(end).skip(start).copied().collect::<Vec<_>>()),
            LongArray(v) => LongArray(v.iter().take(end).skip(start).copied().collect::<Vec<_>>()),
            List(v) => {
                let mut list = VList::new();
                for value in v.into_iter().take(end).skip(start) {
                    let ty = value.tag();

                    if !list.try_push(value) {
                        panic!(
                            "tried to insert {ty:?} into list of type {:?}",
                            list.element_tag()
                        )
                    }
                }

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
        use jaq_core::Error;
        use valence_nbt::Value::*;

        Ok(Val(match self.0 {
            ByteArray(v) => {
                let mut list = VList::new();
                for value in v.iter().copied().map(Byte).map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            let ty = value.0.tag();

                            if !list.try_push(value.0) {
                                return opt.fail(Val(ByteArray(v)), |_| {
                                    Exn::from(jaq_core::Error::str(format_args!(
                                        "tried to insert {ty:?} into list of type {:?}",
                                        list.element_tag()
                                    )))
                                });
                            }
                        }
                        Err(e) => {
                            return opt.fail(Val(ByteArray(v)), |_| e);
                        }
                    }
                }
                valence_nbt::Value::List(list)
            }
            IntArray(v) => {
                let mut list = VList::new();
                for value in v.iter().copied().map(Int).map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            let ty = value.0.tag();

                            if !list.try_push(value.0) {
                                return opt.fail(Val(IntArray(v)), |_| {
                                    Exn::from(jaq_core::Error::str(format_args!(
                                        "tried to insert {ty:?} into list of type {:?}",
                                        list.element_tag()
                                    )))
                                });
                            }
                        }
                        Err(e) => {
                            return opt.fail(Val(IntArray(v)), |_| e);
                        }
                    }
                }
                valence_nbt::Value::List(list)
            }
            LongArray(v) => {
                let mut list = VList::new();
                for value in v.iter().copied().map(Long).map(Val).flat_map(f) {
                    match value {
                        Ok(value) => {
                            let ty = value.0.tag();

                            if !list.try_push(value.0) {
                                return opt.fail(Val(LongArray(v)), |_| {
                                    Exn::from(jaq_core::Error::str(format_args!(
                                        "tried to insert {ty:?} into list of type {:?}",
                                        list.element_tag()
                                    )))
                                });
                            }
                        }
                        Err(e) => {
                            return opt.fail(Val(LongArray(v)), |_| e);
                        }
                    }
                }
                valence_nbt::Value::List(list)
            }
            List(v) => {
                let mut list = VList::new();
                for value in v.iter().map(|e| Val(e.into())).flat_map(f) {
                    match value {
                        Ok(value) => {
                            let ty = value.0.tag();

                            if !list.try_push(value.0) {
                                return opt.fail(Val(List(v)), |_| {
                                    Exn::from(jaq_core::Error::str(format_args!(
                                        "tried to insert {ty:?} into list of type {:?}",
                                        list.element_tag()
                                    )))
                                });
                            }
                        }
                        Err(e) => {
                            return opt.fail(Val(List(v)), |_| e);
                        }
                    }
                }
                valence_nbt::Value::List(list)
            }
            Compound(v) => {
                let mut map = valence_nbt::Compound::with_capacity(v.len());
                for e in v
                    .into_iter()
                    .filter_map(|(k, v)| f(Val(v)).next().map(|v| Ok::<_, Exn<_>>((k, v?))))
                {
                    let (k, v) = e?;
                    map.insert(k, v.0);
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
        use jaq_core::Error;
        use valence_nbt::Value::*;

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
                        v.remove(idx);
                        Ok(Val(ByteArray(v)))
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
                match f(Val(e.into())).next().transpose() {
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
        use valence_nbt::Value::*;

        !matches!(self, Val(Byte(0)))
    }

    fn into_string(self) -> Self {
        Val(valence_nbt::Value::String(format!("{self}")))
    }
}

impl jaq_std::ValT for Val {
    fn into_seq<S: FromIterator<Self>>(self) -> Result<S, Self> {
        use valence_nbt::Value::*;

        match self.0 {
            ByteArray(a) => Ok(a.iter().copied().map(Byte).map(Val).collect()),
            IntArray(a) => Ok(a.iter().copied().map(Int).map(Val).collect()),
            LongArray(a) => Ok(a.iter().copied().map(Long).map(Val).collect()),
            List(a) => Ok(a.into_iter().map(Val).collect()),
            _ => Err(self),
        }
    }

    fn is_int(&self) -> bool {
        use valence_nbt::Value::*;
        matches!(self.0, Byte(_) | Short(_) | Int(_) | Long(_))
    }

    fn as_isize(&self) -> Option<isize> {
        match self.0 {
            valence_nbt::Value::Byte(n) => Some(n as isize),
            valence_nbt::Value::Short(n) => Some(n as isize),
            valence_nbt::Value::Int(n) => Some(n as isize),
            valence_nbt::Value::Long(n) => n.try_into().ok(),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            valence_nbt::Value::Byte(n) => Some(n as f64),
            valence_nbt::Value::Short(n) => Some(n as f64),
            valence_nbt::Value::Int(n) => Some(n as f64),
            valence_nbt::Value::Long(n) => Some(n as f64),
            valence_nbt::Value::Float(n) => Some(n as f64),
            valence_nbt::Value::Double(n) => Some(n),
            _ => None,
        }
    }

    fn is_utf8_str(&self) -> bool {
        matches!(self.0, valence_nbt::Value::String(_))
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            valence_nbt::Value::String(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    fn as_sub_str(&self, sub: &[u8]) -> Self {
        if let valence_nbt::Value::String(s) = &self.0
            && let Some(range) = s.as_bytes().subslice_range(sub)
        {
            Val(valence_nbt::Value::String(s[range].to_string()))
        } else {
            self.clone()
        }
    }

    fn from_utf8_bytes(b: impl AsRef<[u8]> + Send + 'static) -> Self {
        Val(valence_nbt::Value::String(
            std::str::from_utf8(b.as_ref()).unwrap().to_string(),
        ))
    }
}
