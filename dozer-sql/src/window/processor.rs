use crate::errors::PipelineError;
use dozer_core::channels::ProcessorChannelForwarder;
use dozer_core::dozer_log::storage::Object;
use dozer_core::epoch::Epoch;
use dozer_core::node::{PortHandle, Processor};
use dozer_core::DEFAULT_PORT_HANDLE;
use dozer_types::errors::internal::BoxedError;
use dozer_types::types::{Operation, OperationWithId};

use super::operator::WindowType;

#[derive(Debug)]
pub struct WindowProcessor {
    _id: String,
    window: WindowType,
}

impl WindowProcessor {
    pub fn new(id: String, window: WindowType, _checkpoint_data: Option<Vec<u8>>) -> Self {
        Self { _id: id, window }
    }
}

impl Processor for WindowProcessor {
    fn commit(&self, _epoch: &Epoch) -> Result<(), BoxedError> {
        Ok(())
    }

    fn process(
        &mut self,
        _from_port: PortHandle,
        op: OperationWithId,
        fw: &mut dyn ProcessorChannelForwarder,
    ) -> Result<(), BoxedError> {
        match op.op {
            Operation::Delete { old } => {
                let records = self
                    .window
                    .execute(old)
                    .map_err(PipelineError::WindowError)?;
                for record in records {
                    fw.send(
                        OperationWithId::without_id(Operation::Delete { old: record }),
                        DEFAULT_PORT_HANDLE,
                    );
                }
            }
            Operation::Insert { new } => {
                let records = self
                    .window
                    .execute(new)
                    .map_err(PipelineError::WindowError)?;
                for record in records {
                    fw.send(
                        OperationWithId::without_id(Operation::Insert { new: record }),
                        DEFAULT_PORT_HANDLE,
                    );
                }
            }
            Operation::Update { old, new } => {
                self.process(
                    DEFAULT_PORT_HANDLE,
                    OperationWithId::without_id(Operation::Delete { old }),
                    fw,
                )?;

                self.process(
                    DEFAULT_PORT_HANDLE,
                    OperationWithId::without_id(Operation::Insert { new }),
                    fw,
                )?;
            }
            Operation::BatchInsert { new } => {
                let mut records = vec![];
                for record in new {
                    records.extend(
                        self.window
                            .execute(record)
                            .map_err(PipelineError::WindowError)?,
                    );
                }
                fw.send(
                    OperationWithId::without_id(Operation::BatchInsert { new: records }),
                    DEFAULT_PORT_HANDLE,
                );
            }
        }
        Ok(())
    }

    fn serialize(&mut self, _object: Object) -> Result<(), BoxedError> {
        Ok(())
    }
}
