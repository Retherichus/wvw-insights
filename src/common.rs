use anyhow::Result;

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