use std::{ fs, io };
use std::collections::HashSet;
use std::fs::DirEntry;
use std::path::Path;
use std::sync::Mutex;
use lazy_static::lazy_static;
use rayon::prelude::{ IntoParallelRefIterator, ParallelIterator };
use thiserror::Error;
use phf::phf_map;
use std::arch::global_asm;
use ez_logging::println;
use debug_print::{
    debug_print as dprint,
    debug_println as dprintln,
    debug_eprint as deprint,
    debug_eprintln as deprintln,
};
mod logging;
mod cpi_actions;
mod api;



fn main() {
    ez_logging::init();
    cpi_actions::test();

    //let input_dir: &str = "./";
    //try_compile(input_dir).expect("Could not compile");
    api::main();
}