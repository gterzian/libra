// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::syntax::ParseError;
use std::{convert::TryInto, fmt};

// FIXME: This is a simplified version of the lexer generated by lalrpop.
// It should be replaced with something sane.

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tok {
    EOF = -1,
    AccountAddressValue = 0,
    U64Value = 1,
    NameValue = 2,
    NameBeginTyValue = 3,
    DotNameValue = 4,
    ByteArrayValue = 5,
    Exclaim = 6,
    ExclaimEqual = 7,
    Percent = 8,
    Amp = 9,
    AmpAmp = 10,
    AmpMut = 11,
    LParen = 12,
    RParen = 13,
    Star = 14,
    Plus = 15,
    Comma = 16,
    Minus = 17,
    Period = 18,
    Slash = 19,
    Colon = 20,
    Semicolon = 21,
    Less = 22,
    LessEqual = 23,
    Equal = 24,
    EqualEqual = 25,
    Greater = 26,
    GreaterEqual = 27,
    Caret = 28,
    Underscore = 29,
    Abort = 30,
    Acquires = 31,
    Address = 32,
    As = 33,
    Assert = 34,
    Bool = 35,
    BorrowGlobal = 36,
    BorrowGlobalMut = 37,
    Break = 38,
    Bytearray = 39,
    Continue = 40,
    Copy = 41,
    CreateAccount = 42,
    Else = 43,
    Exists = 44,
    False = 45,
    Freeze = 46,
    GetGasRemaining = 47,
    GetTxnGasUnitPrice = 48,
    GetTxnMaxGasUnits = 49,
    GetTxnPublicKey = 50,
    GetTxnSender = 51,
    GetTxnSequenceNumber = 52,
    If = 53,
    Import = 54,
    Let = 55,
    Loop = 56,
    Main = 57,
    Module = 58,
    Modules = 59,
    Move = 60,
    MoveFrom = 61,
    MoveToSender = 62,
    Native = 63,
    Public = 64,
    Resource = 65,
    Return = 66,
    Script = 67,
    Struct = 68,
    True = 69,
    U64 = 70,
    Unrestricted = 71,
    While = 72,
    LBrace = 73,
    Pipe = 74,
    PipePipe = 75,
    RBrace = 76,
}

impl Tok {
    fn from_usize(value: usize) -> Tok {
        match value {
            0 => Tok::AccountAddressValue,
            1 => Tok::U64Value,
            2 => Tok::NameValue,
            3 => Tok::NameBeginTyValue,
            4 => Tok::DotNameValue,
            5 => Tok::ByteArrayValue,
            6 => Tok::Exclaim,
            7 => Tok::ExclaimEqual,
            8 => Tok::Percent,
            9 => Tok::Amp,
            10 => Tok::AmpAmp,
            11 => Tok::AmpMut,
            12 => Tok::LParen,
            13 => Tok::RParen,
            14 => Tok::Star,
            15 => Tok::Plus,
            16 => Tok::Comma,
            17 => Tok::Minus,
            18 => Tok::Period,
            19 => Tok::Slash,
            20 => Tok::Colon,
            21 => Tok::Semicolon,
            22 => Tok::Less,
            23 => Tok::LessEqual,
            24 => Tok::Equal,
            25 => Tok::EqualEqual,
            26 => Tok::Greater,
            27 => Tok::GreaterEqual,
            28 => Tok::Caret,
            29 => Tok::Underscore,
            30 => Tok::Abort,
            31 => Tok::Acquires,
            32 => Tok::Address,
            33 => Tok::As,
            34 => Tok::Assert,
            35 => Tok::Bool,
            36 => Tok::BorrowGlobal,
            37 => Tok::BorrowGlobalMut,
            38 => Tok::Break,
            39 => Tok::Bytearray,
            40 => Tok::Continue,
            41 => Tok::Copy,
            42 => Tok::CreateAccount,
            43 => Tok::Else,
            44 => Tok::Exists,
            45 => Tok::False,
            46 => Tok::Freeze,
            47 => Tok::GetGasRemaining,
            48 => Tok::GetTxnGasUnitPrice,
            49 => Tok::GetTxnMaxGasUnits,
            50 => Tok::GetTxnPublicKey,
            51 => Tok::GetTxnSender,
            52 => Tok::GetTxnSequenceNumber,
            53 => Tok::If,
            54 => Tok::Import,
            55 => Tok::Let,
            56 => Tok::Loop,
            57 => Tok::Main,
            58 => Tok::Module,
            59 => Tok::Modules,
            60 => Tok::Move,
            61 => Tok::MoveFrom,
            62 => Tok::MoveToSender,
            63 => Tok::Native,
            64 => Tok::Public,
            65 => Tok::Resource,
            66 => Tok::Return,
            67 => Tok::Script,
            68 => Tok::Struct,
            69 => Tok::True,
            70 => Tok::U64,
            71 => Tok::Unrestricted,
            72 => Tok::While,
            73 => Tok::LBrace,
            74 => Tok::Pipe,
            75 => Tok::PipePipe,
            76 => Tok::RBrace,
            _ => panic!("Unknown token value: {}", value),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Token<'input>(pub Tok, pub &'input str);
impl<'a> fmt::Display for Token<'a> {
    fn fmt<'f>(&self, formatter: &mut fmt::Formatter<'f>) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self.1, formatter)
    }
}

pub struct LexerBuilder {
    regex_vec: Vec<regex::Regex>,
}

impl LexerBuilder {
    pub fn new() -> LexerBuilder {
        let regex_vec = vec![
            regex::Regex::new("^(0[Xx][0-9A-Fa-f]+)").unwrap(),
            regex::Regex::new("^([0-9]+)").unwrap(),
            regex::Regex::new("^([\\$A-Z_a-z][\\$0-9A-Z_a-z]*)").unwrap(),
            regex::Regex::new("^([\\$A-Z_a-z][\\$0-9A-Z_a-z]*<)").unwrap(),
            regex::Regex::new("^([\\$A-Z_a-z][\\$0-9A-Z_a-z]*\\.[\\$A-Z_a-z][\\$0-9A-Z_a-z]*)")
                .unwrap(),
            regex::Regex::new("^(h\"[0-9A-Fa-f]*\")").unwrap(),
            regex::Regex::new("^(!)").unwrap(),
            regex::Regex::new("^(!=)").unwrap(),
            regex::Regex::new("^(%)").unwrap(),
            regex::Regex::new("^(\\&)").unwrap(),
            regex::Regex::new("^(\\&\\&)").unwrap(),
            regex::Regex::new("^(\\&mut )").unwrap(),
            regex::Regex::new("^(\\()").unwrap(),
            regex::Regex::new("^(\\))").unwrap(),
            regex::Regex::new("^(\\*)").unwrap(),
            regex::Regex::new("^(\\+)").unwrap(),
            regex::Regex::new("^(,)").unwrap(),
            regex::Regex::new("^(\\-)").unwrap(),
            regex::Regex::new("^(\\.)").unwrap(),
            regex::Regex::new("^(/)").unwrap(),
            regex::Regex::new("^(:)").unwrap(),
            regex::Regex::new("^(;)").unwrap(),
            regex::Regex::new("^(<)").unwrap(),
            regex::Regex::new("^(<=)").unwrap(),
            regex::Regex::new("^(=)").unwrap(),
            regex::Regex::new("^(==)").unwrap(),
            regex::Regex::new("^(>)").unwrap(),
            regex::Regex::new("^(>=)").unwrap(),
            regex::Regex::new("^(\\^)").unwrap(),
            regex::Regex::new("^(_)").unwrap(),
            regex::Regex::new("^(abort)").unwrap(),
            regex::Regex::new("^(acquires)").unwrap(),
            regex::Regex::new("^(address)").unwrap(),
            regex::Regex::new("^(as)").unwrap(),
            regex::Regex::new("^(assert\\()").unwrap(),
            regex::Regex::new("^(bool)").unwrap(),
            regex::Regex::new("^(borrow_global<)").unwrap(),
            regex::Regex::new("^(borrow_global_mut<)").unwrap(),
            regex::Regex::new("^(break)").unwrap(),
            regex::Regex::new("^(bytearray)").unwrap(),
            regex::Regex::new("^(continue)").unwrap(),
            regex::Regex::new("^(copy\\()").unwrap(),
            regex::Regex::new("^(create_account)").unwrap(),
            regex::Regex::new("^(else)").unwrap(),
            regex::Regex::new("^(exists<)").unwrap(),
            regex::Regex::new("^(false)").unwrap(),
            regex::Regex::new("^(freeze)").unwrap(),
            regex::Regex::new("^(get_gas_remaining)").unwrap(),
            regex::Regex::new("^(get_txn_gas_unit_price)").unwrap(),
            regex::Regex::new("^(get_txn_max_gas_units)").unwrap(),
            regex::Regex::new("^(get_txn_public_key)").unwrap(),
            regex::Regex::new("^(get_txn_sender)").unwrap(),
            regex::Regex::new("^(get_txn_sequence_number)").unwrap(),
            regex::Regex::new("^(if)").unwrap(),
            regex::Regex::new("^(import)").unwrap(),
            regex::Regex::new("^(let)").unwrap(),
            regex::Regex::new("^(loop)").unwrap(),
            regex::Regex::new("^(main)").unwrap(),
            regex::Regex::new("^(module)").unwrap(),
            regex::Regex::new("^(modules:)").unwrap(),
            regex::Regex::new("^(move\\()").unwrap(),
            regex::Regex::new("^(move_from<)").unwrap(),
            regex::Regex::new("^(move_to_sender<)").unwrap(),
            regex::Regex::new("^(native)").unwrap(),
            regex::Regex::new("^(public)").unwrap(),
            regex::Regex::new("^(resource)").unwrap(),
            regex::Regex::new("^(return)").unwrap(),
            regex::Regex::new("^(script:)").unwrap(),
            regex::Regex::new("^(struct)").unwrap(),
            regex::Regex::new("^(true)").unwrap(),
            regex::Regex::new("^(u64)").unwrap(),
            regex::Regex::new("^(unrestricted)").unwrap(),
            regex::Regex::new("^(while)").unwrap(),
            regex::Regex::new("^(\\{)").unwrap(),
            regex::Regex::new("^(\\|)").unwrap(),
            regex::Regex::new("^(\\|\\|)").unwrap(),
            regex::Regex::new("^(\\})").unwrap(),
        ];
        LexerBuilder { regex_vec }
    }
    pub fn lexer<'input, 'builder>(&'builder self, s: &'input str) -> Lexer<'input, 'builder> {
        Lexer {
            text: s,
            consumed: 0,
            previous_end: 0,
            regex_vec: &self.regex_vec,
            token: (0, Token(Tok::EOF, ""), 0),
        }
    }
}

pub struct Lexer<'input, 'builder> {
    text: &'input str,
    consumed: usize,
    previous_end: usize,
    regex_vec: &'builder Vec<regex::Regex>,
    pub token: (usize, Token<'input>, usize),
}

impl<'input, 'builder> Lexer<'input, 'builder> {
    pub fn peek(&self) -> Tok {
        (self.token.1).0
    }

    pub fn content(&self) -> &str {
        (self.token.1).1
    }

    pub fn start_loc(&self) -> usize {
        self.token.0
    }

    pub fn previous_end_loc(&self) -> usize {
        self.previous_end
    }

    pub fn advance(&mut self) -> Result<(), ParseError<usize, Token<'input>, failure::Error>> {
        self.previous_end = self.token.2;
        let text = self.text.trim_start();
        let whitespace = self.text.len() - text.len();
        let start_offset = self.consumed + whitespace;
        if text.is_empty() {
            self.text = text;
            self.consumed = start_offset;
            self.token = (start_offset, Token(Tok::EOF, ""), start_offset);
            Ok(())
        } else {
            let mut longest_match = 0;
            let mut index: Option<usize> = None;
            for (i, token_exp) in self.regex_vec.iter().enumerate() {
                if let Some(m) = token_exp.find(text) {
                    let len = m.end();
                    if len >= longest_match {
                        longest_match = len;
                        index = Some(i.try_into().unwrap());
                    }
                }
            }
            match index {
                None => Err(ParseError::InvalidToken {
                    location: start_offset,
                }),
                Some(index) => {
                    let result = &text[..longest_match];
                    let remaining = &text[longest_match..];
                    let end_offset = start_offset + longest_match;
                    self.text = remaining;
                    self.consumed = end_offset;
                    self.token = (
                        start_offset,
                        Token(Tok::from_usize(index), result),
                        end_offset,
                    );
                    Ok(())
                }
            }
        }
    }
}