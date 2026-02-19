// src/types.rs

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardType {
    Unknown,
    Mifare1K,
    Mifare4K,
    MifareUltralight,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyType {
    KeyA,
    KeyB,
}

#[derive(Debug)]
pub enum Error<E> {
    Spi(E),
    Timeout,
    ParityError,
    CRCError,
    Collision,
    NoCard,
}
