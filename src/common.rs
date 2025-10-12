use anyhow::Result;

pub const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

#[derive(Debug)]
pub struct WorkerMessage {
    pub index: usize,
    pub payload: WorkerType,
}

impl WorkerMessage {
    pub fn upload_result(index: usize, result: Result<String>) -> Self {
        Self {
            index,
            payload: WorkerType::UploadResult(result),
        }
    }
}

#[derive(Debug)]
pub enum WorkerType {
    UploadResult(Result<String>),
}
