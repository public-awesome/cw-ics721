#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

pub enum ZipOptional<A, B> {
    Some { a: A, b: B },
    None { a: A },
}

impl<A, B> Iterator for ZipOptional<A, B>
where
    A: Iterator,
    B: Iterator,
{
    type Item = (A::Item, Option<B::Item>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::None { a } => a.next().map(|a| (a, None)),
            Self::Some { a, b } => {
                let a = a.next()?;
                let b = b.next()?;
                Some((a, Some(b)))
            }
        }
    }
}

pub fn zip_optional<A, B>(a: A, b: Option<B>) -> ZipOptional<A::IntoIter, B::IntoIter>
where
    A: IntoIterator,
    B: IntoIterator,
{
    match b {
        Some(b) => ZipOptional::Some {
            a: a.into_iter(),
            b: b.into_iter(),
        },
        None => ZipOptional::None { a: a.into_iter() },
    }
}

pub trait Zippable<B>
where
    Self: Sized,
    B: IntoIterator,
{
    fn zip_optional(self, b: Option<B>) -> ZipOptional<Self, B::IntoIter>;
}

impl<I, B> Zippable<B> for I
where
    I: Iterator,
    B: IntoIterator,
{
    fn zip_optional(self, b: Option<B>) -> ZipOptional<Self, B::IntoIter> {
        crate::zip_optional(self, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_optional_some() {
        let a = vec![1, 2];
        let b = Some(vec![1, 2]);

        let mut zipped = zip_optional(a, b);
        assert_eq!(zipped.next().unwrap(), (1, Some(1)));
        assert_eq!(zipped.next().unwrap(), (2, Some(2)));
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_none() {
        let a = vec![1, 2];

        let mut zipped = zip_optional(a, None::<Vec<i32>>);
        assert_eq!(zipped.next().unwrap(), (1, None));
        assert_eq!(zipped.next().unwrap(), (2, None));
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_empty() {
        let a = Vec::<i32>::new();

        let mut zipped = zip_optional(a, None::<Vec<i32>>);
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_iter_none() {
        let mut zipped = vec![1, 2].into_iter().zip_optional(None::<Vec<i32>>);
        assert_eq!(zipped.next().unwrap(), (1, None));
        assert_eq!(zipped.next().unwrap(), (2, None));
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_iter_some() {
        let mut zipped = vec![1, 2].into_iter().zip_optional(Some(vec![1, 2]));
        assert_eq!(zipped.next().unwrap(), (1, Some(1)));
        assert_eq!(zipped.next().unwrap(), (2, Some(2)));
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_iter_empty() {
        let mut zipped = Vec::<i32>::new().into_iter().zip_optional(None::<Vec<i32>>);
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_longer_non_optional() {
        let mut zipped = vec![1, 2].into_iter().zip_optional(Some(vec![1]));
        assert_eq!(zipped.next().unwrap(), (1, Some(1)));
        assert_eq!(zipped.next(), None);
    }

    #[test]
    fn test_zip_optional_longer_optional() {
        let mut zipped = vec![1].into_iter().zip_optional(Some(vec![1, 2]));
        assert_eq!(zipped.next().unwrap(), (1, Some(1)));
        assert_eq!(zipped.next(), None);
    }
}
