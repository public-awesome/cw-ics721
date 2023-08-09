use ics721::ibc::Ics721Ibc;
use sg_std::StargazeMsgWrapper;

use crate::state::SgIcs721Contract;

impl Ics721Ibc<StargazeMsgWrapper> for SgIcs721Contract {}
