#![feature(proc_macro_hygiene)]

use drone_core::{bitfield::Bitfield, reg::prelude::*, token::Token};
use std::mem::size_of;

use drone_core::reg;

use crate::test_block::{test_reg::Val, TestReg};

reg! {
    /// Test reg doc attribute
    #[doc = "test reg attribute"]
    pub mod TEST_BLOCK TEST_REG;

    0xDEAD_BEEF 0x20 0xBEEF_CACE RReg WReg;

    TEST_BIT { 0 1 RRRegField WWRegField }
    TEST_BITS { 1 3 RRRegField WWRegField }
}

reg::tokens! {
    /// Test index doc attribute
    #[doc = "test index attribute"]
    pub macro reg_tokens;
    crate;
    crate;

    /// Test block doc attribute
    #[doc = "test block attribute"]
    pub mod TEST_BLOCK {
        TEST_REG;
    }
}

reg_tokens! {
    /// Test index doc attribute
    #[doc = "test index attribute"]
    pub struct Regs;
}

#[test]
fn reg_default_val() {
    assert_eq!(unsafe { TestReg::<Srt>::take() }.default_val().bits(), 0xBEEF_CACE);
}

#[test]
fn size_of_reg() {
    assert_eq!(size_of::<TestReg<Urt>>(), 0);
    assert_eq!(size_of::<TestReg<Srt>>(), 0);
    assert_eq!(size_of::<TestReg<Crt>>(), 0);
}

#[test]
fn size_of_reg_val() {
    assert_eq!(size_of::<Val>(), 4);
}
