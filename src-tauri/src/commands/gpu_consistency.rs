//! Tauri command for GPU consistency matrix.

use crate::core::gpu_consistency::{self, GpuMatrix};
use crate::data::Db;
use crate::error::UecmResult;
use tauri::State;

#[tauri::command]
pub fn get_gpu_consistency_matrix(db: State<'_, Db>) -> UecmResult<GpuMatrix> {
    gpu_consistency::build_matrix(&db)
}
