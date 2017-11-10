#![feature(decl_macro)]

extern crate drone;

use drone::reg;
use drone::reg::prelude::*;
use std as core;

reg!(TEST_REG 0xDEAD_BEEF 0x20 0xBEEF_CACE);

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn main() {
  assert_send::<TestReg<Sr>>();
  assert_sync::<TestReg<Sr>>();
  assert_send::<TestReg<Cr>>();
  assert_sync::<TestReg<Cr>>();
}
