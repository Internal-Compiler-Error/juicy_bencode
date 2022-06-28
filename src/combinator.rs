#![allow(unused)]

use nom::{branch::alt, bytes::complete::{tag, take_while}, character::complete::{char, digit0, i64, u64}, combinator::recognize, sequence::{terminated, tuple}, IResult, Err};
use std::collections::BTreeMap;

extern crate derive_more;

use nom::bytes::complete::{take, take_while1};
use nom::combinator::{complete, map};
use nom::error::{ErrorKind, ParseError};
use nom::multi::many1;
use nom::ParseTo;
use nom::sequence::{delimited, pair, preceded};

#[derive(Debug)]
pub enum BencodeSchemaErrorKinds {
    DictNotInLexicographicalOrder,
    NomInternal(ErrorKind),
}

#[derive(Debug)]
pub struct BencodeSchemaError {
    message: String,
    kind: BencodeSchemaErrorKinds,
}

impl BencodeSchemaError {
    pub fn new(message: String, kind: BencodeSchemaErrorKinds) -> Self {
        Self {
            message,
            kind,
        }
    }
}

impl ParseError<&[u8]> for BencodeSchemaError {
    fn from_error_kind(input: &[u8], kind: ErrorKind) -> Self {
        let message = format!("{:?}:\t{:?}\n", kind, input);
        let kind = BencodeSchemaErrorKinds::NomInternal(kind);
        Self {
            message,
            kind,
        }
    }

    fn append(input: &[u8], kind: ErrorKind, other: Self) -> Self {
        let message = format!("{}{:?}:\t{:?}\n", other.message, kind, input);
        let kind = BencodeSchemaErrorKinds::NomInternal(kind);
        Self {
            message,
            kind,
        }
    }
}

fn is_non_zero_num(c: u8) -> bool {
    [b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9'].contains(&c)
}


pub fn parse_bencode_num(input: &[u8]) -> IResult<&[u8], &[u8]>
{
    let zero = complete(tag("0"));
    let minus_sign = tag("-");

    // positive case, any digits without leading zeros
    // the function can't be clone for some reason, welp
    let positive1 = recognize(pair(take_while1(is_non_zero_num), digit0));
    let positive2 = recognize(pair(take_while1(is_non_zero_num), digit0));


    // negative case
    let negative = recognize(pair(minus_sign, positive1));

    delimited(tag("i"),
              alt((positive2, negative, zero)),
              tag("e"))(input)
}


pub fn parse_bencode_string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (str, length) = u64(input)?;

    preceded(tag(":"), take(length))(str)
}

pub fn parse_bencode_list(input: &[u8]) -> IResult<&[u8], Vec<BencodeItemView>> {
    let list_elems = many1(bencode_value);

    delimited(
        tag("l"),
        list_elems,
        tag("e"),
    )(input)
}


pub fn parse_bencode_dict(input: &[u8]) -> IResult<&[u8], BTreeMap<&[u8], BencodeItemView>> {
    let key_value = many1(pair(
        parse_bencode_string, bencode_value,
    ));

    let (remaining, key_value_pairs) = delimited(tag("d"), key_value, tag("e"))(input)?;


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

pub fn bencode_value(input: &[u8]) -> IResult<&[u8], BencodeItemView> {
    let to_int = map(parse_bencode_num, |int_pattern| BencodeItemView::Integer(int_pattern.parse_to().unwrap()));
    let to_byte_str = map(parse_bencode_string, |byte_slice| BencodeItemView::ByteString(byte_slice));
    let to_list = map(parse_bencode_list, |items| BencodeItemView::List(items));
    let to_dict = map(parse_bencode_dict, |dict| BencodeItemView::Dictionary(dict));

    alt((to_int, to_byte_str, to_list, to_dict))(input)
}


#[derive(Debug, Ord, Clone, PartialOrd, Eq, PartialEq, Hash)]
pub enum BencodeItemView<'a> {
    // todo: technically the specification doesn't say any limits on the integer size, need to switch
    // to an infinite size one
    Integer(i64),
    ByteString(&'a [u8]),
    List(Vec<BencodeItemView<'a>>),
    Dictionary(BTreeMap<&'a [u8], BencodeItemView<'a>>),
}


#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use crate::combinator::{BencodeItemView, parse_bencode_dict, parse_bencode_list, parse_bencode_num, parse_bencode_string};

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

        let expected = vec![BencodeItemView::ByteString(b"spam"), BencodeItemView::Integer(42)];
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
}
