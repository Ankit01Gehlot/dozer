use dozer_cache::cache::CacheManagerOptions;
use dozer_core::{
    checkpoint::{CheckpointFactoryOptions, CheckpointOptions},
    executor::ExecutorOptions,
};
use dozer_types::{
    constants::DEFAULT_DEFAULT_MAX_NUM_RECORDS,
    models::{
        app_config::{
            default_app_buffer_size, default_commit_size, default_commit_timeout,
            default_error_threshold, default_persist_queue_capacity,
        },
        config::{default_cache_dir, default_cache_max_map_size, Config},
    },
};
use std::time::Duration;

fn get_cache_max_map_size(config: &Config) -> u64 {
    config
        .cache_max_map_size
        .unwrap_or_else(default_cache_max_map_size)
}

fn get_commit_time_threshold(config: &Config) -> Duration {
    Duration::from_millis(
        config
            .app
            .commit_timeout
            .unwrap_or_else(default_commit_timeout),
    )
}

fn get_buffer_size(config: &Config) -> u32 {
    config
        .app
        .app_buffer_size
        .unwrap_or_else(default_app_buffer_size)
}

fn get_commit_size(config: &Config) -> u32 {
    config.app.commit_size.unwrap_or_else(default_commit_size)
}

fn get_error_threshold(config: &Config) -> u32 {
    config
        .app
        .error_threshold
        .unwrap_or_else(default_error_threshold)
}

pub fn get_checkpoint_options(config: &Config) -> CheckpointOptions {
    let app = &config.app;
    CheckpointOptions {
        data_storage: app.data_storage.clone(),
    }
}

fn get_checkpoint_factory_options(config: &Config) -> CheckpointFactoryOptions {
    CheckpointFactoryOptions {
        persist_queue_capacity: config
            .app
            .persist_queue_capacity
            .unwrap_or_else(default_persist_queue_capacity)
            as usize,
    }
}

pub fn get_executor_options(config: &Config) -> ExecutorOptions {
    ExecutorOptions {
        commit_sz: get_commit_size(config),
        channel_buffer_sz: get_buffer_size(config) as usize,
        commit_time_threshold: get_commit_time_threshold(config),
        error_threshold: Some(get_error_threshold(config)),
        checkpoint_factory_options: get_checkpoint_factory_options(config),
    }
}

pub fn get_cache_manager_options(config: &Config) -> CacheManagerOptions {
    CacheManagerOptions {
        path: Some(
            config
                .cache_dir
                .clone()
                .unwrap_or_else(default_cache_dir)
                .into(),
        ),
        max_size: get_cache_max_map_size(config) as usize,
        ..CacheManagerOptions::default()
    }
}

pub fn get_default_max_num_records(config: &Config) -> usize {
    config
        .api
        .default_max_num_records
        .unwrap_or(DEFAULT_DEFAULT_MAX_NUM_RECORDS)
}
