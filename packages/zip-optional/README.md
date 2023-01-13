# Zip Optional

An iterator type for zipping with an optional iterable.

When the iterable being zipped has no value (i.e. is `None`), the
initial iterable is effectively zipped with `std::iter::repeat(None)`.

```rust
# use zip_optional::zip_optional;

let a = vec![1, 2];

let mut zipped = zip_optional(a, None::<Vec<i32>>);
assert_eq!(zipped.next().unwrap(), (1, None));
assert_eq!(zipped.next().unwrap(), (2, None));
assert_eq!(zipped.next(), None);
```

When the iterable being zipped has a value, the result of a sequence
of `Some(_)` which contains the items in the iterable being zipped
with.

```rust
# use zip_optional::zip_optional;

let a = vec![1, 2];
let b = Some(vec![1, 2]);

let mut zipped = zip_optional(a, b);
assert_eq!(zipped.next().unwrap(), (1, Some(1)));
assert_eq!(zipped.next().unwrap(), (2, Some(2)));
assert_eq!(zipped.next(), None);
```

The provided iterator may also be used inline with other iteration
methods, like so:

```rust
# use zip_optional::Zippable;

let mut zipped = vec![1, 2].into_iter().zip_optional(Some(vec![1, 2]));
assert_eq!(zipped.next().unwrap(), (1, Some(1)));
assert_eq!(zipped.next().unwrap(), (2, Some(2)));
assert_eq!(zipped.next(), None);
```
