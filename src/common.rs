use anyhow::Result;

#[derive(Debug)]
pub struct WorkerMessage {
    pub index: usize,
    pub payload: WorkerType,
}

#[derive(Debug)]
pub enum WorkerType {
    UploadResult(Result<String>),
}

impl WorkerMessage {
    pub fn upload_result(index: usize, result: Result<String>) -> Self {
        Self {
            index,
            payload: WorkerType::UploadResult(result),
        }
    }
}