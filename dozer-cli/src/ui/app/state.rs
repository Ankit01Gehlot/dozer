use std::{collections::HashMap, sync::Arc, thread::JoinHandle};

use clap::Parser;

use dozer_api::shutdown::{self, ShutdownReceiver, ShutdownSender};
use dozer_cache::dozer_log::camino::Utf8Path;
use dozer_core::{app::AppPipeline, dag_schemas::DagSchemas, Dag};
use dozer_sql::builder::statement_to_pipeline;
use dozer_tracing::{Labels, LabelsAndProgress};
use dozer_types::{
    grpc_types::{
        app_ui::{AppUi, AppUiResponse, BuildResponse, BuildStatus, ConnectResponse, RunRequest},
        contract::{DotResponse, ProtoResponse},
        types::SchemasResponse,
    },
    log::info,
    models::{
        api_config::{ApiConfig, AppGrpcOptions, GrpcApiOptions, RestApiOptions},
        api_security::ApiSecurity,
        endpoint::{ApiEndpoint, Endpoint, EndpointKind},
        flags::Flags,
    },
};
use tempdir::TempDir;
use tokio::{runtime::Runtime, sync::RwLock};

use super::AppUIError;
use crate::{
    cli::{init_config, init_dozer, types::Cli},
    errors::OrchestrationError,
    pipeline::{EndpointLog, EndpointLogKind, PipelineBuilder},
    simple::{helper::validate_config, Contract, SimpleOrchestrator},
};
struct DozerAndContract {
    dozer: SimpleOrchestrator,
    contract: Option<Contract>,
}

pub struct ShutdownAndTempDir {
    shutdown: ShutdownSender,
    _temp_dir: TempDir,
}

#[derive(Debug)]
pub enum BroadcastType {
    Start,
    Success,
    Failed(String),
}
pub struct AppUIState {
    dozer: RwLock<Option<DozerAndContract>>,
    run_thread: RwLock<Option<ShutdownAndTempDir>>,
    error_message: RwLock<Option<String>>,
    sender: RwLock<Option<tokio::sync::broadcast::Sender<ConnectResponse>>>,
}

impl Default for AppUIState {
    fn default() -> Self {
        Self::new()
    }
}
impl AppUIState {
    pub fn new() -> Self {
        Self {
            dozer: RwLock::new(None),
            run_thread: RwLock::new(None),
            sender: RwLock::new(None),
            error_message: RwLock::new(None),
        }
    }

    async fn create_contract_if_missing(&self) -> Result<(), AppUIError> {
        let mut dozer_and_contract_lock = self.dozer.write().await;
        if let Some(dozer_and_contract) = dozer_and_contract_lock.as_mut() {
            if dozer_and_contract.contract.is_none() {
                let contract = create_contract(dozer_and_contract.dozer.clone()).await?;
                dozer_and_contract.contract = Some(contract);
            }
        }
        Ok(())
    }

    pub async fn set_sender(&self, sender: tokio::sync::broadcast::Sender<ConnectResponse>) {
        *self.sender.write().await = Some(sender);
    }

    pub async fn broadcast(&self, broadcast_type: BroadcastType) {
        let sender = self.sender.read().await;
        info!("Broadcasting state: {:?}", broadcast_type);
        if let Some(sender) = sender.as_ref() {
            let res = match broadcast_type {
                BroadcastType::Start => ConnectResponse {
                    app_ui: None,
                    build: Some(BuildResponse {
                        status: BuildStatus::BuildStart as i32,
                        message: None,
                    }),
                },
                BroadcastType::Failed(msg) => ConnectResponse {
                    app_ui: None,
                    build: Some(BuildResponse {
                        status: BuildStatus::BuildFailed as i32,
                        message: Some(msg),
                    }),
                },
                BroadcastType::Success => {
                    let res = self.get_current().await;
                    ConnectResponse {
                        app_ui: Some(res),
                        build: None,
                    }
                }
            };
            let _ = sender.send(res);
        }
    }

    pub async fn set_error_message(&self, error_message: Option<String>) {
        *self.error_message.write().await = error_message;
    }

    pub async fn build(&self, runtime: Arc<Runtime>) -> Result<(), AppUIError> {
        // Taking lock to ensure that we don't have multiple builds running at the same time
        let mut lock = self.dozer.write().await;

        let cli = Cli::parse();
        let (config, _) = init_config(
            cli.config_paths.clone(),
            cli.config_token.clone(),
            cli.config_overrides.clone(),
            cli.ignore_pipe,
        )
        .await?;

        let dozer = init_dozer(runtime, config, Default::default())?;

        let contract = create_contract(dozer.clone()).await;
        *lock = Some(DozerAndContract {
            dozer,
            contract: match &contract {
                Ok(contract) => Some(contract.clone()),
                Err(_) => None,
            },
        });
        if let Err(e) = &contract {
            self.set_error_message(Some(e.to_string())).await;
        } else {
            self.set_error_message(None).await;
        }

        contract
            .map(|_| ())
            .map_err(|e| AppUIError::OrchestrationError(Box::new(e)))
    }
    pub async fn get_current(&self) -> AppUiResponse {
        let dozer = self.dozer.read().await;
        let app = dozer.as_ref().map(|dozer| {
            let config = &dozer.dozer.config;
            let connections_in_source: Vec<String> = config
                .sources
                .iter()
                .map(|source| source.connection.clone())
                .collect::<std::collections::HashSet<String>>()
                .into_iter()
                .collect();

            let endpoints = dozer
                .dozer
                .config
                .sinks
                .iter()
                .map(|endpoint| endpoint.table_name.clone())
                .collect();

            let enable_api_security = std::env::var("DOZER_MASTER_SECRET")
                .ok()
                .map(ApiSecurity::Jwt)
                .as_ref()
                .or(dozer.dozer.config.api.api_security.as_ref())
                .is_some();
            AppUi {
                app_name: dozer.dozer.config.app_name.clone(),
                connections: connections_in_source,
                endpoints,
                enable_api_security,
            }
        });
        AppUiResponse {
            initialized: app.is_some(),
            running: self.run_thread.read().await.is_some(),
            error_message: self.error_message.read().await.as_ref().cloned(),
            app,
        }
    }

    pub async fn get_endpoints_schemas(&self) -> Result<SchemasResponse, AppUIError> {
        self.create_contract_if_missing().await?;
        let dozer = self.dozer.read().await;
        let contract = get_contract(&dozer)?;
        Ok(SchemasResponse {
            schemas: contract.get_endpoints_schemas(),
            errors: HashMap::new(),
        })
    }
    pub async fn get_source_schemas(
        &self,
        connection_name: String,
    ) -> Result<SchemasResponse, AppUIError> {
        self.create_contract_if_missing().await?;
        let dozer = self.dozer.read().await;
        let contract = get_contract(&dozer)?;

        contract
            .get_source_schemas(&connection_name)
            .ok_or(AppUIError::ConnectionNotFound(connection_name))
            .map(|schemas| SchemasResponse {
                schemas,
                errors: HashMap::new(),
            })
    }

    pub async fn get_graph_schemas(&self) -> Result<SchemasResponse, AppUIError> {
        self.create_contract_if_missing().await?;
        let dozer = self.dozer.read().await;
        let contract = get_contract(&dozer)?;

        Ok(SchemasResponse {
            schemas: contract.get_graph_schemas(),
            errors: HashMap::new(),
        })
    }

    pub async fn generate_dot(&self) -> Result<DotResponse, AppUIError> {
        self.create_contract_if_missing().await?;
        let dozer = self.dozer.read().await;
        let contract = get_contract(&dozer)?;

        Ok(DotResponse {
            dot: contract.generate_dot(),
        })
    }

    pub async fn get_protos(&self) -> Result<ProtoResponse, AppUIError> {
        let dozer = self.dozer.read().await;
        let contract = get_contract(&dozer)?;
        let (protos, libraries) = contract.get_protos()?;

        Ok(ProtoResponse { protos, libraries })
    }

    pub async fn run(&self, request: RunRequest) -> Result<Labels, AppUIError> {
        let dozer = self.dozer.read().await;
        let dozer = &dozer.as_ref().ok_or(AppUIError::NotInitialized)?.dozer;
        // kill if a handle already exists
        self.stop().await?;
        let temp_dir = TempDir::new("dozer_app_local")?;
        let temp_dir_path = temp_dir.path().to_str().unwrap();

        let labels: Labels = [("dozer_app_local_id", uuid::Uuid::new_v4().to_string())]
            .into_iter()
            .collect();
        let (shutdown_sender, shutdown_receiver) = shutdown::new(&dozer.runtime);
        let _handle = run(
            dozer.clone(),
            labels.clone(),
            request,
            shutdown_receiver,
            temp_dir_path,
        )?;

        let mut lock = self.run_thread.write().await;
        if let Some(shutdown_and_tempdir) = lock.take() {
            shutdown_and_tempdir.shutdown.shutdown();
        }
        let shutdown_and_tempdir = ShutdownAndTempDir {
            shutdown: shutdown_sender,
            _temp_dir: temp_dir,
        };
        *lock = Some(shutdown_and_tempdir);
        Ok(labels)
    }

    pub async fn stop(&self) -> Result<(), AppUIError> {
        let mut lock = self.run_thread.write().await;
        if let Some(shutdown_and_tempdir) = lock.take() {
            shutdown_and_tempdir.shutdown.shutdown();
            shutdown_and_tempdir._temp_dir.close()?;
        }
        *lock = None;
        Ok(())
    }
    pub async fn get_api_token(&self, ttl: Option<i32>) -> Result<Option<String>, AppUIError> {
        let dozer: tokio::sync::RwLockReadGuard<'_, Option<DozerAndContract>> =
            self.dozer.read().await;
        let dozer = &dozer.as_ref().ok_or(AppUIError::NotInitialized)?.dozer;
        let generated_token = dozer.generate_token(ttl).ok();
        Ok(generated_token)
    }
}

fn get_contract(dozer_and_contract: &Option<DozerAndContract>) -> Result<&Contract, AppUIError> {
    dozer_and_contract
        .as_ref()
        .ok_or(AppUIError::NotInitialized)?
        .contract
        .as_ref()
        .ok_or(AppUIError::NotInitialized)
}

pub async fn create_contract(dozer: SimpleOrchestrator) -> Result<Contract, OrchestrationError> {
    let dag = create_dag(&dozer).await?;
    let version = dozer.config.version;
    let schemas = DagSchemas::new(dag).await?;
    let contract = Contract::new(
        version as usize,
        &schemas,
        &dozer.config.connections,
        &dozer.config.sinks,
        // We don't care about API generation options here. They are handled in `run_all`.
        false,
        true,
    )?;
    Ok(contract)
}

pub async fn create_dag(dozer: &SimpleOrchestrator) -> Result<Dag, OrchestrationError> {
    let endpoint_and_logs = dozer
        .config
        .sinks
        .iter()
        // We're not really going to run the pipeline, so we don't create logs.
        .map(|endpoint| EndpointLog {
            table_name: endpoint.table_name.clone(),
            kind: match &endpoint.config.clone() {
                EndpointKind::Api(_) => EndpointLogKind::Dummy,
                EndpointKind::Dummy => EndpointLogKind::Dummy,
                EndpointKind::Aerospike(config) => EndpointLogKind::Aerospike {
                    config: config.clone(),
                },
                EndpointKind::Clickhouse(config) => EndpointLogKind::Clickhouse {
                    config: config.clone(),
                },
                EndpointKind::Oracle(config) => EndpointLogKind::Oracle {
                    config: config.clone(),
                },
            },
        })
        .collect();
    let builder = PipelineBuilder::new(
        &dozer.config.connections,
        &dozer.config.sources,
        dozer.config.sql.as_deref(),
        endpoint_and_logs,
        Default::default(),
        Flags::default(),
        &dozer.config.udfs,
    );
    let (_shutdown_sender, shutdown_receiver) = shutdown::new(&dozer.runtime);
    builder.build(&dozer.runtime, shutdown_receiver).await
}

fn run(
    dozer: SimpleOrchestrator,
    labels: Labels,
    request: RunRequest,
    shutdown_receiver: ShutdownReceiver,
    temp_dir: &str,
) -> Result<JoinHandle<()>, OrchestrationError> {
    let dozer = get_dozer_run_instance(dozer, labels, request, temp_dir)?;

    validate_config(&dozer.config)?;
    let runtime = dozer.runtime.clone();

    let handle: JoinHandle<()> = std::thread::spawn(move || {
        runtime.block_on(async move { dozer.run_all(shutdown_receiver, false).await.unwrap() });
    });

    Ok(handle)
}

fn get_dozer_run_instance(
    mut dozer: SimpleOrchestrator,
    labels: Labels,
    req: RunRequest,
    temp_dir: &str,
) -> Result<SimpleOrchestrator, AppUIError> {
    match req.request {
        Some(dozer_types::grpc_types::app_ui::run_request::Request::Sql(req)) => {
            let context = statement_to_pipeline(
                &req.sql,
                &mut AppPipeline::new(dozer.config.flags.clone().into()),
                None,
                dozer.config.udfs.clone(),
                dozer.runtime.clone(),
            )
            .map_err(AppUIError::PipelineError)?;

            //overwrite sql
            dozer.config.sql = Some(req.sql);

            dozer.config.sinks = vec![];
            let tables = context.output_tables_map.keys().collect::<Vec<_>>();
            for table in tables {
                let endpoint = Endpoint {
                    table_name: table.to_string(),
                    config: EndpointKind::Api(ApiEndpoint {
                        path: format!("/{}", table),
                        index: Default::default(),
                        conflict_resolution: Default::default(),
                        version: Default::default(),
                        log_reader_options: Default::default(),
                    }),
                };
                dozer.config.sinks.push(endpoint);
            }
        }
        Some(dozer_types::grpc_types::app_ui::run_request::Request::Source(req)) => {
            dozer.config.sql = None;
            dozer.config.sinks = vec![];
            let endpoint = req.source;
            dozer.config.sinks.push(Endpoint {
                table_name: endpoint.to_string(),
                config: EndpointKind::Api(ApiEndpoint {
                    path: format!("/{}", endpoint),
                    index: Default::default(),
                    conflict_resolution: Default::default(),
                    version: Default::default(),
                    log_reader_options: Default::default(),
                }),
            });
        }
        None => {}
    };

    override_api_config(&mut dozer.config.api);

    dozer.config.flags.enable_app_checkpoints = Some(false);

    dozer.config.home_dir = Some(temp_dir.to_string());
    dozer.config.cache_dir = Some(AsRef::<Utf8Path>::as_ref(temp_dir).join("cache").into());

    dozer.labels = LabelsAndProgress::new(labels, false);

    Ok(dozer)
}

fn override_api_config(api: &mut ApiConfig) {
    override_rest_config(&mut api.rest);
    override_grpc_config(&mut api.grpc);
    override_app_grpc_config(&mut api.app_grpc);
    api.pgwire.enabled = Some(true);
}

fn override_rest_config(rest: &mut RestApiOptions) {
    rest.host = Some("0.0.0.0".to_string());
    rest.port = Some(62885);
    rest.cors = Some(true);
    rest.enabled = Some(true);
    rest.enable_sql = Some(true);
}

fn override_grpc_config(grpc: &mut GrpcApiOptions) {
    grpc.host = Some("0.0.0.0".to_string());
    grpc.port = Some(62887);
    grpc.cors = Some(true);
    grpc.web = Some(true);
    grpc.enabled = Some(true);
}

fn override_app_grpc_config(app_grpc: &mut AppGrpcOptions) {
    app_grpc.port = Some(62997);
    app_grpc.host = Some("0.0.0.0".to_string());
}
