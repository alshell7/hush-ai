//! Windows on ARM: tract 0.19, the inference library of libDF 0.5.6, has no assembly for this
//! target, so DeepFilterNet 3 is not built here and [`DeepFilter::load`] says so. Hush offers
//! NVIDIA RE-USE instead (see [`DEEP_FILTER_AVAILABLE`]).

use crate::{Enhancer, WorkerScheduling};
use eyre::{eyre, Result};
use std::path::Path;

/// DeepFilterNet 3 runs at 48 kHz.
pub const DEEP_FILTER_RATE: u32 = 48_000;

/// No value of this type exists on this target.
pub enum DeepFilter {}

impl DeepFilter {
    pub fn load(_path: impl AsRef<Path>) -> Result<Self> {
        Err(eyre!(
            "DeepFilterNet 3 is not available on Windows on ARM yet; choose NVIDIA RE-USE"
        ))
    }

    pub fn set_worker_scheduling(&mut self, _scheduling: WorkerScheduling) {
        match *self {}
    }

    pub fn sample_rate(&self) -> u32 {
        DEEP_FILTER_RATE
    }

    pub fn frame(&self) -> usize {
        match *self {}
    }

    pub fn latency(&self) -> usize {
        match *self {}
    }

    pub fn reset(&mut self) {
        match *self {}
    }

    pub fn prepare(&mut self) -> Result<()> {
        match *self {}
    }

    pub fn process_into(&mut self, _input: &[f32], _out: &mut Vec<f32>) -> Result<()> {
        match *self {}
    }

    pub fn flush(&mut self) -> Result<Vec<f32>> {
        match *self {}
    }
}

impl Enhancer for DeepFilter {
    fn sample_rate(&self) -> u32 {
        DEEP_FILTER_RATE
    }
    fn frame(&self) -> usize {
        match *self {}
    }
    fn latency(&self) -> usize {
        match *self {}
    }
    fn reset(&mut self) {
        match *self {}
    }
    fn prepare(&mut self) -> Result<()> {
        match *self {}
    }
    fn process_into(&mut self, _input: &[f32], _out: &mut Vec<f32>) -> Result<()> {
        match *self {}
    }
    fn flush(&mut self) -> Result<Vec<f32>> {
        match *self {}
    }
    fn worker_scheduling(&self) -> WorkerScheduling {
        match *self {}
    }
}
