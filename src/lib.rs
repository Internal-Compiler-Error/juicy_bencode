#![allow(unused)]
//! ![](https://img.shields.io/docsrs/juicy_bencode)
//! ![](https://img.shields.io/crates/v/juicy_bencode)
//! ![](https://img.shields.io/crates/l/juicy_bencode)
//!
//! A little parser for [bencode](https://www.bittorrent.org/beps/bep_0003.html#bencoding) using the
//! Nom library. **Nom eats input byte by byte, and bencode is such juicy input!**
//!
//! Bencode allows for 4 kinds of values:
//! 1. integers
//! 2. byte strings
//! 3. lists
//! 4. dictionaries
//!
//! Unlike JSON, bencode doesn't say how to encode stuff in general, but in practice, you should
//! just//! parse a bencode blob as a dictionary (just like JSON). Although, the individual parsing
//! functions are provided.
//!
//! For more information about bencode, you're encouraged to read the specification. It's less than
//! 200 words long!

use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while1},
    character::complete::{char, digit0, i64, u64},
    multi::many1,
    combinator::{recognize, complete, map},
    sequence::{terminated, delimited, pair, preceded},
    Err, IResult, ParseTo, Parser,
};
use std::collections::BTreeMap;

fn is_non_zero_num(c: u8) -> bool {
    [b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9'].contains(&c)
}


/// Parse out a bencode integer, conforming to bencode specification, leading 0s and negative 0 are
/// rejected. Since the bencode specification places no limit on the range of the integers, the
/// function will only give out string slices and leave the conversion choice to the user.`
///
/// # Note
/// Although the functions are exposed directly, it's unsuitable to be used directly in most cases,
/// it's provided for quick and dirty convenience only.
pub fn parse_bencode_num(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let zero = complete(tag("0"));
    let minus_sign = tag("-");

    // positive case, any digits without leading zeros
    // the function can't be clone for some reason, welp
    let positive1 = recognize(pair(take_while1(is_non_zero_num), digit0));
    let positive2 = recognize(pair(take_while1(is_non_zero_num), digit0));

    // negative case
    let negative = recognize(pair(minus_sign, positive1));

    delimited(tag("i"), alt((positive2, negative, zero)), tag("e")).parse(input)
}

/// Parse out a bencode string, note that bencode strings are not equivalent to Rust strings since
/// bencode places no limit what encoding it uses, hence it's more appreciate to call them byte
/// strings
///
/// # Note
/// Although the functions are exposed directly, it's unsuitable to be used directly in most cases,
/// it's provided for quick and dirty convenience only.
pub fn parse_bencode_string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (str, length) = u64(input)?;

    preceded(tag(":"), take(length)).parse(str)
}

/// Parse out bencode list, technically, bencode places not restriction on if the list items are
/// homogeneous, meaning a list could contain both integers and strings.
///
/// # Note
/// Although the functions are exposed directly, it's unsuitable to be used directly in most cases,
/// it's provided for quick and dirty convenience only.
pub fn parse_bencode_list(input: &[u8]) -> IResult<&[u8], Vec<BencodeItemView<'_>>> {
    let list_elems = many1(bencode_value);

    delimited(tag("l"), list_elems, tag("e")).parse(input)
}


/// Main entry for the parser (for all practical purposes, a blob of bencode is consist of key value
/// pairs). It parses out a bencode dictionary, bencode places no restriction on the homogeneity of
/// dictionary pairs.
pub fn parse_bencode_dict(input: &[u8]) -> IResult<&[u8], BTreeMap<&[u8], BencodeItemView<'_>>> {
    let key_value = many1(pair(parse_bencode_string, bencode_value));

    let (remaining, key_value_pairs) = delimited(tag("d"), key_value, tag("e")).parse(input)?;

    let dict = key_value_pairs
        .into_iter()
        .fold(BTreeMap::new(), |mut acc, x| {
            acc.insert(x.0, x.1);
            acc
        });

    Ok((remaining, dict))

    // TODO: bencode requires the keys of the dictionary to be in lexicographical order, maybe this
    // isn't the best place to handle this
    //
    // // somehow the Vec::is_sorted requires nightly as of 1.61, this is so ghetto
    //
    // let mut sorted = key_value_pairs.clone();
    // sorted.sort_unstable_by_key(|elem| elem.0);
    // let sorted_keys = sorted.iter().map(|x| x.0);
    //
    //
    // if !key_value_pairs
    //     .iter()
    //     .map(|x| x.0)
    //     .zip(sorted_keys)
    //     .all(|pair| pair.0 == pair.1) {
    //     return Err::Failure(BencodeSchemaError::new("Nothing is actually broken about your dict, but the bencode specification states all keys must appear in lexicographical order".into_string(), BencodeSchemaErrorKinds::DictNotInLexicographicalOrder));
    // }
}

/// Top level combinator for choosing an appreciate strategy for parsing out a bencode item
fn bencode_value(input: &[u8]) -> IResult<&[u8], BencodeItemView<'_>> {
    let to_int = map(parse_bencode_num, |int_pattern| {
        BencodeItemView::Integer(int_pattern.parse_to().unwrap())
    });
    let to_byte_str = map(parse_bencode_string, |byte_slice| {
        BencodeItemView::ByteString(byte_slice)
    });
    let to_list = map(parse_bencode_list, BencodeItemView::List);
    let to_dict = map(parse_bencode_dict,  BencodeItemView::Dictionary);

    alt((to_int, to_byte_str, to_list, to_dict)).parse(input)
}

/// Representation of bencode blobs as a tree. The lifetime is tied to the text in memory, achieving
/// *almost zero copy*. This is perhaps unsuitable for large bencode blobs since the entire blob may
/// not fit inside the memory.
///
/// See [`BencodeItem`] for an owned version of this tree.
#[derive(Debug, Ord, Clone, PartialOrd, Eq, PartialEq, Hash)]
pub enum BencodeItemView<'a> {
    // TODO: technically the specification doesn't say any limits on the integer size, need to switch
    // to an infinite size one
    /// Bencode integers are represented as i64 for now, technically this is not to specification
    /// since no range limit is specified in the bencode document.
    Integer(i64),

    /// Bencode strings are not guaranteed to be UTF-8, thus using a byte slice
    ByteString(&'a [u8]),

    /// Bencode lists, note lists may not be homogeneous
    List(Vec<BencodeItemView<'a>>),

    /// Bencode dictionary, note lists may not be homogeneous. Bencode dictionary by specification
    /// must be lexicographically sorted, BTree preserves ordering
    Dictionary(BTreeMap<&'a [u8], BencodeItemView<'a>>),
}

/// Owned version of [`BencodeItemView`], for when the parsed value must outlive the input
/// buffer. Obtained via [`BencodeItemView::into_owned`].
#[derive(Debug, Ord, Clone, PartialOrd, Eq, PartialEq, Hash)]
pub enum BencodeItem {
    /// Same as [`BencodeItemView::Integer`]
    Integer(i64),

    /// Byte string copied out of the input buffer
    ByteString(Vec<u8>),

    /// List of owned items
    List(Vec<BencodeItem>),

    /// Dictionary with owned keys and owned values
    Dictionary(BTreeMap<Vec<u8>, BencodeItem>),
}

impl<'a> BencodeItemView<'a> {
    /// Recursively copies all borrowed data, producing an owned [`BencodeItem`] that is
    /// independent of the input buffer's lifetime.
    pub fn into_owned(self) -> BencodeItem {
        match self {
            BencodeItemView::Integer(int) => BencodeItem::Integer(int),
            BencodeItemView::ByteString(bytes) => BencodeItem::ByteString(bytes.to_vec()),
            BencodeItemView::List(items) => {
                BencodeItem::List(items.into_iter().map(BencodeItemView::into_owned).collect())
            }
            BencodeItemView::Dictionary(dict) => BencodeItem::Dictionary(
                dict.into_iter()
                    .map(|(key, value)| (key.to_vec(), value.into_owned()))
                    .collect(),
            ),
        }
    }
}

impl<'a> From<BencodeItemView<'a>> for BencodeItem {
    fn from(view: BencodeItemView<'a>) -> Self {
        view.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_valid_be_num() {
        let (_, parsed) = parse_bencode_num(b"i0e").unwrap();
        assert_eq!(parsed, b"0");
    }

    #[test]
    fn positive_number_without_leading_zero_is_valid() {
        let (_, parsed) = parse_bencode_num(b"i124223e").unwrap();
        assert_eq!(parsed, b"124223");
    }

    #[test]
    fn positive_number_with_leading_zero_is_rejected() {
        let res = parse_bencode_num(b"i0001e");
        assert!(res.is_err());
    }

    #[test]
    fn negative_number_without_leading_zero_is_valid() {
        let (_, parsed) = parse_bencode_num(b"i-121241e").unwrap();
        assert_eq!(parsed, b"-121241")
    }

    #[test]
    fn negative_zero_is_not_allowed() {
        let parsed = parse_bencode_num(b"i-0e");
        assert!(parsed.is_err())
    }

    #[test]
    fn positive_number_with_zeroes_in_between_is_valid() {
        let (_, parsed) = parse_bencode_num(b"i700454e").unwrap();
        assert_eq!(parsed, b"700454");
    }

    #[test]
    fn negative_number_with_zeroes_in_between_is_valid() {
        let (_, parsed) = parse_bencode_num(b"i-6004e").unwrap();
        assert_eq!(parsed, b"-6004")
    }

    #[test]
    fn naked_numbers_are_not_bencode_numbers() {
        let parsed = parse_bencode_num(b"8232");
        assert!(parsed.is_err())
    }

    #[test]
    fn negative_number_with_leading_zeroes_is_not_allowed() {
        let parsed = parse_bencode_num(b"i-0001213e");
        assert!(parsed.is_err())
    }

    #[test]
    fn letters_are_not_be_numbers() {
        let parsed = parse_bencode_num(b"iabcedfge");
        assert!(parsed.is_err());
    }

    #[test]
    fn naked_string_is_not_bencode_string() {
        let parsed = parse_bencode_string(b"string!");
        assert!(parsed.is_err());
    }

    #[test]
    fn bencode_string_takes_exact_length() {
        let (_, parsed) = parse_bencode_string(b"4:spam").unwrap();
        assert_eq!(parsed, b"spam");
    }

    #[test]
    fn strings_shorter_than_declaration_is_not_allowed() {
        let parsed = parse_bencode_string(b"4:spa");
        assert!(parsed.is_err());
    }

    #[test]
    fn bencode_list_eats_all_inputs() {
        let (remaining, parsed) = parse_bencode_list(b"l4:spami42ee").unwrap();

        let expected = vec![
            BencodeItemView::ByteString(b"spam"),
            BencodeItemView::Integer(42),
        ];
        assert_eq!(expected, parsed);
        assert_eq!(remaining, b"");
    }

    #[test]
    fn bencode_dict_eats_all_inputs() {
        let (remaining, parsed) = parse_bencode_dict(b"d3:bar4:spam3:fooi42ee").unwrap();

        let mut expected = BTreeMap::new();
        expected.insert(b"bar".as_slice(), BencodeItemView::ByteString(b"spam"));
        expected.insert(b"foo".as_slice(), BencodeItemView::Integer(42));

        assert_eq!(expected, parsed);
        assert_eq!(remaining, b"");
    }

    #[test]
    fn integer_view_into_owned() {
        let view = BencodeItemView::Integer(42);
        assert_eq!(BencodeItem::Integer(42), view.into_owned());
    }

    #[test]
    fn byte_string_view_into_owned() {
        let view = BencodeItemView::ByteString(b"spam");
        assert_eq!(BencodeItem::ByteString(b"spam".to_vec()), view.into_owned());
    }

    #[test]
    fn parsed_list_views_into_owned() {
        // the list entry point yields Vec<BencodeItemView>, convert each element
        let (_, views) = parse_bencode_list(b"l4:spami42ee").unwrap();
        let owned: Vec<BencodeItem> = views.into_iter().map(BencodeItemView::into_owned).collect();

        let expected = vec![
            BencodeItem::ByteString(b"spam".to_vec()),
            BencodeItem::Integer(42),
        ];
        assert_eq!(expected, owned);
    }

    #[test]
    fn parsed_dict_into_owned_via_from() {
        let (_, view) = parse_bencode_dict(b"d3:bar4:spam3:fooi42ee").unwrap();
        // the dict entry point yields the raw BTreeMap, wrap it in the Dictionary view variant
        let owned: BencodeItem = BencodeItemView::Dictionary(view).into(); // exercises the From impl

        let mut expected = BTreeMap::new();
        expected.insert(b"bar".to_vec(), BencodeItem::ByteString(b"spam".to_vec()));
        expected.insert(b"foo".to_vec(), BencodeItem::Integer(42));
        assert_eq!(BencodeItem::Dictionary(expected), owned);
    }

    #[test]
    fn nested_structures_convert_recursively() {
        // list inside dict
        let (_, view) = parse_bencode_dict(b"d4:listl4:spam4:eggsee").unwrap();
        let owned = BencodeItemView::Dictionary(view).into_owned();

        let mut expected = BTreeMap::new();
        expected.insert(
            b"list".to_vec(),
            BencodeItem::List(vec![
                BencodeItem::ByteString(b"spam".to_vec()),
                BencodeItem::ByteString(b"eggs".to_vec()),
            ]),
        );
        assert_eq!(BencodeItem::Dictionary(expected), owned);

        // dict inside list
        let (_, views) = parse_bencode_list(b"ld3:fooi42eee").unwrap();
        let owned: Vec<BencodeItem> = views.into_iter().map(BencodeItemView::into_owned).collect();

        let mut inner = BTreeMap::new();
        inner.insert(b"foo".to_vec(), BencodeItem::Integer(42));
        assert_eq!(vec![BencodeItem::Dictionary(inner)], owned);
    }
}
