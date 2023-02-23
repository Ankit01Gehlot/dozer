use dozer_api::generator::protoc::generator::ProtoGenerator;
use dozer_api::grpc::internal_grpc::pipeline_response::ApiEvent;
use dozer_api::grpc::internal_grpc::PipelineResponse;
use dozer_api::grpc::types_helper;
use dozer_cache::cache::expression::QueryExpression;
use dozer_cache::cache::index::get_primary_key;
use dozer_cache::cache::{CacheManager, RwCache};
use dozer_core::epoch::Epoch;
use dozer_core::errors::{ExecutionError, SinkError};
use dozer_core::node::{PortHandle, Sink, SinkFactory};
use dozer_core::record_store::RecordReader;
use dozer_core::storage::lmdb_storage::SharedTransaction;
use dozer_core::DEFAULT_PORT_HANDLE;
use dozer_sql::pipeline::builder::SchemaSQLContext;
use dozer_types::crossbeam::channel::Sender;
use dozer_types::indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use dozer_types::log::debug;
use dozer_types::models::api_endpoint::{ApiEndpoint, ApiIndex};
use dozer_types::models::api_security::ApiSecurity;
use dozer_types::models::flags::Flags;
use dozer_types::types::FieldType;
use dozer_types::types::{IndexDefinition, Operation, Schema, SchemaIdentifier};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub fn attach_progress(multi_pb: Option<MultiProgress>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    multi_pb.as_ref().map(|m| m.add(pb.clone()));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            // For more spinners check out the cli-spinners project:
            // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ]),
    );
    pb
}
#[derive(Debug, Clone)]
pub struct CacheSinkSettings {
    api_dir: PathBuf,
    flags: Option<Flags>,
    api_security: Option<ApiSecurity>,
}
impl CacheSinkSettings {
    pub fn new(api_dir: PathBuf, flags: Option<Flags>, api_security: Option<ApiSecurity>) -> Self {
        Self {
            api_dir,
            flags,
            api_security,
        }
    }
}
#[derive(Debug)]
pub struct CacheSinkFactory {
    cache_manager: Arc<dyn CacheManager>,
    api_endpoint: ApiEndpoint,
    notifier: Option<Sender<PipelineResponse>>,
    multi_pb: MultiProgress,
    settings: CacheSinkSettings,
}

impl CacheSinkFactory {
    pub fn new(
        cache_manager: Arc<dyn CacheManager>,
        api_endpoint: ApiEndpoint,
        notifier: Option<Sender<PipelineResponse>>,
        multi_pb: MultiProgress,
        settings: CacheSinkSettings,
    ) -> Result<Self, ExecutionError> {
        Ok(Self {
            cache_manager,
            api_endpoint,
            notifier,
            multi_pb,
            settings,
        })
    }

    fn get_output_schema(
        &self,
        mut input_schemas: HashMap<PortHandle, Schema>,
    ) -> Result<(Schema, Vec<IndexDefinition>), ExecutionError> {
        debug_assert!(input_schemas.len() == 1);
        let mut schema = input_schemas
            .remove(&DEFAULT_PORT_HANDLE)
            .expect("Input schema should be on default port");

        // Generated Cache index based on api_index
        let configured_index = create_primary_indexes(
            &schema,
            &self.api_endpoint.index.to_owned().unwrap_or_default(),
        )?;
        // Generated schema in SQL
        let upstream_index = schema.primary_index.clone();

        let index = match (configured_index.is_empty(), upstream_index.is_empty()) {
            (true, true) => vec![],
            (true, false) => upstream_index,
            (false, true) => configured_index,
            (false, false) => {
                if !upstream_index.eq(&configured_index) {
                    return Err(ExecutionError::MismatchPrimaryKey {
                        endpoint_name: self.api_endpoint.name.clone(),
                        expected: get_field_names(&schema, &upstream_index),
                        actual: get_field_names(&schema, &configured_index),
                    });
                }
                configured_index
            }
        };

        schema.primary_index = index;

        schema.identifier = Some(SchemaIdentifier {
            id: DEFAULT_PORT_HANDLE as u32,
            version: 1,
        });

        // Automatically create secondary indexes
        let secondary_indexes = schema
            .fields
            .iter()
            .enumerate()
            .flat_map(|(idx, f)| match f.typ {
                // Create sorted inverted indexes for these fields
                FieldType::UInt
                | FieldType::Int
                | FieldType::Float
                | FieldType::Boolean
                | FieldType::Decimal
                | FieldType::Timestamp
                | FieldType::Date
                | FieldType::Point => vec![IndexDefinition::SortedInverted(vec![idx])],

                // Create sorted inverted and full text indexes for string fields.
                FieldType::String => vec![
                    IndexDefinition::SortedInverted(vec![idx]),
                    IndexDefinition::FullText(idx),
                ],

                // Create full text indexes for text fields
                // FieldType::Text => vec![IndexDefinition::FullText(idx)],
                FieldType::Text => vec![],

                // Skip creating indexes
                FieldType::Binary | FieldType::Bson => vec![],
            })
            .collect();
        Ok((schema, secondary_indexes))
    }
}

impl SinkFactory<SchemaSQLContext> for CacheSinkFactory {
    fn get_input_ports(&self) -> Vec<PortHandle> {
        vec![DEFAULT_PORT_HANDLE]
    }

    fn prepare(
        &self,
        input_schemas: HashMap<PortHandle, (Schema, SchemaSQLContext)>,
    ) -> Result<(), ExecutionError> {
        use std::println as stdinfo;
        stdinfo!(
            "SINK: Initializing output schema: {}",
            self.api_endpoint.name
        );

        // Get output schema.
        let (schema, _) = self.get_output_schema(
            input_schemas
                .into_iter()
                .map(|(key, (schema, _))| (key, schema))
                .collect(),
        )?;
        schema.print().printstd();

        // Generate proto files.
        ProtoGenerator::generate(
            &self.settings.api_dir,
            &self.api_endpoint.name,
            &schema,
            &self.settings.api_security,
            &self.settings.flags,
        )
        .map_err(|e| ExecutionError::InternalError(Box::new(e)))?;

        Ok(())
    }

    fn build(
        &self,
        input_schemas: HashMap<PortHandle, Schema>,
    ) -> Result<Box<dyn Sink>, ExecutionError> {
        // Create cache.
        let (schema, secondary_indexes) = self.get_output_schema(input_schemas)?;
        let cache = open_or_create_cache(
            &*self.cache_manager,
            &self.api_endpoint.name,
            schema,
            secondary_indexes,
        )?;

        Ok(Box::new(CacheSink::new(
            cache,
            self.api_endpoint.clone(),
            self.notifier.clone(),
            Some(self.multi_pb.clone()),
        )?))
    }
}

fn create_primary_indexes(
    schema: &Schema,
    api_index: &ApiIndex,
) -> Result<Vec<usize>, ExecutionError> {
    let mut primary_index = Vec::new();
    for name in api_index.primary_key.iter() {
        let idx = schema
            .fields
            .iter()
            .position(|fd| fd.name == name.clone())
            .map_or(Err(ExecutionError::FieldNotFound(name.to_owned())), |p| {
                Ok(p)
            })?;

        primary_index.push(idx);
    }
    Ok(primary_index)
}

fn get_field_names(schema: &Schema, indexes: &[usize]) -> Vec<String> {
    indexes
        .iter()
        .map(|idx| schema.fields[*idx].name.to_owned())
        .collect()
}

fn open_or_create_cache(
    cache_manager: &dyn CacheManager,
    name: &str,
    schema: Schema,
    secondary_indexes: Vec<IndexDefinition>,
) -> Result<Box<dyn RwCache>, ExecutionError> {
    if let Some(cache) = cache_manager.open_rw_cache(name).map_err(|e| {
        ExecutionError::SinkError(SinkError::CacheOpenFailed(name.to_string(), Box::new(e)))
    })? {
        Ok(cache)
    } else {
        let cache = cache_manager
            .create_cache(vec![(name.to_string(), schema, secondary_indexes)])
            .map_err(|e| {
                ExecutionError::SinkError(SinkError::CacheCreateFailed(
                    name.to_string(),
                    Box::new(e),
                ))
            })?;
        cache_manager
            .create_alias(cache.name(), name)
            .map_err(|e| {
                ExecutionError::SinkError(SinkError::CacheCreateAliasFailed {
                    alias: name.to_string(),
                    real_name: cache.name().to_string(),
                    source: Box::new(e),
                })
            })?;
        Ok(cache)
    }
}

#[derive(Debug)]
pub struct CacheSink {
    cache: Box<dyn RwCache>,
    counter: usize,
    api_endpoint: ApiEndpoint,
    pb: ProgressBar,
    notifier: Option<Sender<PipelineResponse>>,
}

impl Sink for CacheSink {
    fn commit(&mut self, _epoch: &Epoch, _tx: &SharedTransaction) -> Result<(), ExecutionError> {
        let endpoint_name = self.api_endpoint.name.clone();
        // Update Counter on commit
        self.pb.set_message(format!(
            "{}: Count: {}",
            self.api_endpoint.name.to_owned(),
            self.counter,
        ));
        self.cache.commit().map_err(|e| {
            ExecutionError::SinkError(SinkError::CacheCommitTransactionFailed(
                endpoint_name,
                Box::new(e),
            ))
        })?;
        Ok(())
    }

    fn process(
        &mut self,
        _from_port: PortHandle,
        op: Operation,
        _tx: &SharedTransaction,
        _reader: &HashMap<PortHandle, Box<dyn RecordReader>>,
    ) -> Result<(), ExecutionError> {
        self.counter += 1;

        let endpoint_name = &self.api_endpoint.name;
        let schema = &self
            .cache
            .get_schema_and_indexes_by_name(endpoint_name)
            .map_err(|_| ExecutionError::SchemaNotInitialized)?
            .0;

        let mapped_op = match op {
            Operation::Delete { mut old } => {
                old.schema_id = schema.identifier;
                let key = get_primary_key(&schema.primary_index, &old.values);
                let version = self.cache.delete(&key).map_err(|e| {
                    ExecutionError::SinkError(SinkError::CacheDeleteFailed(
                        endpoint_name.clone(),
                        Box::new(e),
                    ))
                })?;
                old.version = Some(version);
                types_helper::map_delete_operation(self.api_endpoint.name.clone(), old)
            }
            Operation::Insert { mut new } => {
                new.schema_id = schema.identifier;
                let id = self.cache.insert(&mut new).map_err(|e| {
                    ExecutionError::SinkError(SinkError::CacheInsertFailed(
                        endpoint_name.clone(),
                        Box::new(e),
                    ))
                })?;
                types_helper::map_insert_operation(self.api_endpoint.name.clone(), new, id)
            }
            Operation::Update { mut old, mut new } => {
                old.schema_id = schema.identifier;
                new.schema_id = schema.identifier;
                let key = get_primary_key(&schema.primary_index, &old.values);
                let old_version = self.cache.update(&key, &mut new).map_err(|e| {
                    ExecutionError::SinkError(SinkError::CacheUpdateFailed(
                        endpoint_name.clone(),
                        Box::new(e),
                    ))
                })?;
                old.version = Some(old_version);
                types_helper::map_update_operation(self.api_endpoint.name.clone(), old, new)
            }
        };

        if let Some(notifier) = &self.notifier {
            notifier
                .try_send(PipelineResponse {
                    endpoint: self.api_endpoint.name.clone(),
                    api_event: Some(ApiEvent::Op(mapped_op)),
                })
                .map_err(|e| ExecutionError::InternalError(Box::new(e)))?;
        }

        Ok(())
    }
}

impl CacheSink {
    pub fn new(
        cache: Box<dyn RwCache>,
        api_endpoint: ApiEndpoint,
        notifier: Option<Sender<PipelineResponse>>,
        multi_pb: Option<MultiProgress>,
    ) -> Result<Self, ExecutionError> {
        let query = QueryExpression::with_no_limit();
        let counter = cache.count(&api_endpoint.name, &query).map_err(|e| {
            ExecutionError::SinkError(SinkError::CacheCountFailed(
                api_endpoint.name.clone(),
                Box::new(e),
            ))
        })?;

        debug!(
            "SINK: Initialising CacheSink: {} with count: {}",
            api_endpoint.name, counter
        );
        let pb = attach_progress(multi_pb);
        Ok(Self {
            cache,
            counter,
            api_endpoint,
            pb,
            notifier,
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::test_utils;

    use dozer_cache::cache::{index, CacheManager};
    use dozer_core::node::Sink;
    use dozer_core::storage::lmdb_storage::LmdbEnvironmentManager;
    use dozer_core::DEFAULT_PORT_HANDLE;

    use dozer_types::node::NodeHandle;
    use dozer_types::types::{Field, IndexDefinition, Operation, Record, SchemaIdentifier};
    use std::collections::HashMap;
    use tempdir::TempDir;

    #[test]
    // This test cases covers update of records when primary key changes because of value change in primary_key
    fn update_record_when_primary_changes() {
        let tmp_dir = TempDir::new("example").unwrap();
        let env =
            LmdbEnvironmentManager::create(tmp_dir.path(), "test", Default::default()).unwrap();
        let txn = env.create_txn().unwrap();

        let schema = test_utils::get_schema();
        let secondary_indexes: Vec<IndexDefinition> = schema
            .fields
            .iter()
            .enumerate()
            .map(|(idx, _f)| IndexDefinition::SortedInverted(vec![idx]))
            .collect();

        let (cache_manager, mut sink) = test_utils::init_sink(schema.clone(), secondary_indexes);
        let cache = cache_manager
            .open_ro_cache(sink.cache.name())
            .unwrap()
            .unwrap();

        let initial_values = vec![Field::Int(1), Field::String("Film name old".to_string())];

        let updated_values = vec![
            Field::Int(2),
            Field::String("Film name updated".to_string()),
        ];

        let insert_operation = Operation::Insert {
            new: Record {
                schema_id: Option::from(SchemaIdentifier { id: 1, version: 1 }),
                values: initial_values.clone(),
                version: None,
            },
        };

        let update_operation = Operation::Update {
            old: Record {
                schema_id: Option::from(SchemaIdentifier { id: 1, version: 1 }),
                values: initial_values.clone(),
                version: None,
            },
            new: Record {
                schema_id: Option::from(SchemaIdentifier { id: 1, version: 1 }),
                values: updated_values.clone(),
                version: None,
            },
        };

        sink.process(DEFAULT_PORT_HANDLE, insert_operation, &txn, &HashMap::new())
            .unwrap();
        sink.commit(
            &dozer_core::epoch::Epoch::from(
                0,
                NodeHandle::new(Some(DEFAULT_PORT_HANDLE), "".to_string()),
                0,
                0,
            ),
            &txn,
        )
        .unwrap();

        let key = index::get_primary_key(&schema.primary_index, &initial_values);
        let record = cache.get(&key).unwrap().record;

        assert_eq!(initial_values, record.values);

        sink.process(DEFAULT_PORT_HANDLE, update_operation, &txn, &HashMap::new())
            .unwrap();
        let epoch1 = dozer_core::epoch::Epoch::from(
            0,
            NodeHandle::new(Some(DEFAULT_PORT_HANDLE), "".to_string()),
            0,
            1,
        );
        sink.commit(&epoch1, &txn).unwrap();

        // Primary key with old values
        let key = index::get_primary_key(&schema.primary_index, &initial_values);

        let record = cache.get(&key);

        assert!(record.is_err());

        // Primary key with updated values
        let key = index::get_primary_key(&schema.primary_index, &updated_values);
        let record = cache.get(&key).unwrap().record;

        assert_eq!(updated_values, record.values);
    }
}
